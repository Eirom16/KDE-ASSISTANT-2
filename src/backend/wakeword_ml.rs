//! Wake Word ML con openWakeWord (ONNX Runtime)
//!
//! Replica el pipeline de `openwakeword` (Python):
//! 1. Audio int16 @16kHz en chunks de 1280 samples (80ms)
//! 2. `melspectrogram.onnx` -> mel features (transform x/10+2)
//! 3. `embedding_model.onnx` sobre ventanas de 76 frames -> embedding 96-dim
//! 4. `hey_jarvis_v0.1.onnx` sobre las ultimas 16 embeddings -> score 0..1
//!
//! La libreria ONNX Runtime se carga dinamicamente en runtime:
//! 1. `ORT_DYLIB_PATH` (override explicito)
//! 2. Sistema (`libonnxruntime.so` via ldconfig, ej. `pacman -S onnxruntime`)
//! 3. Copia incluida en el paquete python `onnxruntime` (fallback)
//!
//! Si no se encuentra, retorna un error claro con instrucciones.

use anyhow::{anyhow, bail, Result};
use ort::{session::Session, value::Tensor};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

const CHUNK_SAMPLES: usize = 1280; // 80ms @16kHz
const MEL_WINDOW: usize = 76;
const FEAT_DIM: usize = 96;
const CLASSIFIER_FRAMES: usize = 16;
const MEL_MAX_ROWS: usize = 970; // 10*97
const FEAT_MAX_ROWS: usize = 120;
const RAW_MAX_SAMPLES: usize = 160000; // 10s @16kHz

pub struct OwwDetector {
    mel: Session,
    emb: Session,
    clf: Session,
    raw: VecDeque<f32>, // int16-scale f32, cap 10s
    remainder: Vec<f32>,
    accumulated: usize,
    mel_buf: Vec<[f32; 32]>,  // cap 970, init 76 filas de unos
    feat_buf: Vec<[f32; 96]>, // cap 120
    frames_seen: usize,
    pub threshold: f32,
}

impl OwwDetector {
    /// Crea el detector. Descubre y carga libonnxruntime, luego los 3 modelos.
    pub fn new(models_dir: &Path, threshold: f32) -> Result<Self> {
        ensure_onnx_lib()?;

        let mel_path = models_dir.join("melspectrogram.onnx");
        let emb_path = models_dir.join("embedding_model.onnx");
        let clf_path = models_dir.join("hey_jarvis_v0.1.onnx");
        for p in [&mel_path, &emb_path, &clf_path] {
            if !p.exists() {
                bail!(
                    "modelo openWakeWord no encontrado: {}. Descarga melspectrogram.onnx, embedding_model.onnx y hey_jarvis_v0.1.onnx de https://github.com/dscripka/openWakeWord/releases a {}",
                    p.display(),
                    models_dir.display()
                );
            }
        }

        let mel = Session::builder()
            .map_err(|e| anyhow!("onnx session builder: {e:?}"))?
            .with_intra_threads(1)
            .map_err(|e| anyhow!("onnx intra_threads: {e:?}"))?
            .commit_from_file(&mel_path)
            .map_err(|e| anyhow!("cargando {}: {e:?}", mel_path.display()))?;
        let emb = Session::builder()
            .map_err(|e| anyhow!("onnx session builder: {e:?}"))?
            .with_intra_threads(1)
            .map_err(|e| anyhow!("onnx intra_threads: {e:?}"))?
            .commit_from_file(&emb_path)
            .map_err(|e| anyhow!("cargando {}: {e:?}", emb_path.display()))?;
        let clf = Session::builder()
            .map_err(|e| anyhow!("onnx session builder: {e:?}"))?
            .with_intra_threads(1)
            .map_err(|e| anyhow!("onnx intra_threads: {e:?}"))?
            .commit_from_file(&clf_path)
            .map_err(|e| anyhow!("cargando {}: {e:?}", clf_path.display()))?;

        log::info!("openWakeWord ML listo (hey_jarvis, threshold={threshold})");

        let mut det = Self {
            mel,
            emb,
            clf,
            raw: VecDeque::with_capacity(RAW_MAX_SAMPLES),
            remainder: Vec::new(),
            accumulated: 0,
            mel_buf: vec![[1.0f32; 32]; MEL_WINDOW],
            feat_buf: Vec::with_capacity(FEAT_MAX_ROWS),
            frames_seen: 0,
            threshold,
        };
        // Warmup: pre-llenar buffers con ruido (como el original, que usa
        // embeddings de ruido aleatorio). Sin esto, la primera clasificacion
        // tardaria 16 frames (~1.3s) y se perderian utterances cortas.
        det.warmup();
        Ok(det)
    }

    /// Pre-llena los buffers con ruido de fondo (determinista).
    fn warmup(&mut self) {
        // Ruido pseudoaleatorio +/-1000 (escala int16), como el original.
        let mut state: u32 = 0x12345678;
        let mut noise = Vec::with_capacity(16000 * 2);
        for _ in 0..16000 * 2 {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let v = (state % 2001) as f32 - 1000.0;
            noise.push(v);
        }
        // Alimentar en chunks de 1280 (ignorar scores del warmup)
        for chunk in noise.chunks(CHUNK_SAMPLES) {
            let _ = self.feed_raw(chunk);
        }
        self.frames_seen = 0;
    }

    /// Núcleo de `feed` que opera sobre samples ya en escala int16.
    fn feed_raw(&mut self, samples_int16scale: &[f32]) -> Result<Option<f32>> {
        let mut x: Vec<f32> = Vec::with_capacity(self.remainder.len() + samples_int16scale.len());
        x.extend_from_slice(&self.remainder);
        x.extend_from_slice(samples_int16scale);
        self.remainder.clear();

        // Acumular en chunks de 1280
        let total = self.accumulated + x.len();
        if total < CHUNK_SAMPLES {
            self.accumulated += x.len();
            self.push_raw(&x);
            return Ok(None);
        }

        let remainder = total % CHUNK_SAMPLES;
        let (even, rest) = if remainder != 0 {
            x.split_at(x.len() - remainder)
        } else {
            (x.as_slice(), &[][..])
        };
        self.push_raw(even);
        self.accumulated += even.len();
        self.remainder.extend_from_slice(rest);

        // Solo calcular cuando el acumulado es multiplo exacto de 1280
        if self.accumulated % CHUNK_SAMPLES != 0 {
            return Ok(None);
        }

        let n_chunks = self.accumulated / CHUNK_SAMPLES;

        // Melspectrogram sobre los ultimos (accumulated + 480) samples
        let take = (self.accumulated + 480).min(self.raw.len());
        let seg: Vec<f32> = self.raw.iter().rev().take(take).rev().copied().collect();
        let mel = self.run_mel(&seg)?;
        self.mel_buf.extend(mel);
        if self.mel_buf.len() > MEL_MAX_ROWS {
            let excess = self.mel_buf.len() - MEL_MAX_ROWS;
            self.mel_buf.drain(..excess);
        }

        // Embeddings por cada chunk nuevo (de atras hacia adelante)
        for i in (0..n_chunks).rev() {
            let ndx: isize = if i == 0 {
                self.mel_buf.len() as isize
            } else {
                -(8 * i as isize)
            };
            let end = ndx as usize;
            if end < MEL_WINDOW || end > self.mel_buf.len() {
                continue;
            }
            let mut window = [[0.0f32; 32]; MEL_WINDOW];
            window.copy_from_slice(&self.mel_buf[end - MEL_WINDOW..end]);
            let emb = self.run_emb(&window)?;
            self.feat_buf.push(emb);
        }
        if self.feat_buf.len() > FEAT_MAX_ROWS {
            let excess = self.feat_buf.len() - FEAT_MAX_ROWS;
            self.feat_buf.drain(..excess);
        }

        self.accumulated = 0;

        // Clasificador sobre las ultimas 16 embeddings
        if self.feat_buf.len() < CLASSIFIER_FRAMES {
            return Ok(Some(0.0));
        }
        let score = self.run_clf()?;
        self.frames_seen += 1;
        // Cero las primeras 5 predicciones (warmup, como el original)
        if self.frames_seen < 5 {
            return Ok(Some(0.0));
        }
        Ok(Some(score))
    }

    /// Alimenta audio mono f32 @16kHz. Retorna Some(score) por cada frame
    /// de 80ms completado, None si aun no hay frame completo.
    pub fn feed(&mut self, samples_16k: &[f32]) -> Result<Option<f32>> {
        // Convertir a escala int16 (el modelo espera float32 con valores int16)
        let mut x: Vec<f32> = Vec::with_capacity(samples_16k.len());
        for s in samples_16k {
            x.push((s.clamp(-1.0, 1.0) * 32767.0).round());
        }
        self.feed_raw(&x)
    }

    fn push_raw(&mut self, samples: &[f32]) {
        self.raw.extend(samples.iter().copied());
        while self.raw.len() > RAW_MAX_SAMPLES {
            self.raw.pop_front();
        }
    }

    fn run_mel(&mut self, samples: &[f32]) -> Result<Vec<[f32; 32]>> {
        let n = samples.len();
        let input = Tensor::from_array(([1usize, n], samples.to_vec().into_boxed_slice()))?;
        let outputs = self.mel.run(ort::inputs!["input" => input])?;
        let (_shape, data) = outputs["output"].try_extract_tensor::<f32>()?;
        // Salida [1,1,T,32] -> T filas de 32 con transform x/10+2
        let t = data.len() / 32;
        let mut rows = Vec::with_capacity(t);
        for r in 0..t {
            let mut row = [0.0f32; 32];
            for c in 0..32 {
                row[c] = data[r * 32 + c] / 10.0 + 2.0;
            }
            rows.push(row);
        }
        Ok(rows)
    }

    fn run_emb(&mut self, window: &[[f32; 32]]) -> Result<[f32; 96]> {
        debug_assert_eq!(window.len(), MEL_WINDOW);
        let mut flat = Vec::with_capacity(MEL_WINDOW * 32);
        for row in window {
            flat.extend_from_slice(row);
        }
        let input = Tensor::from_array((
            [1usize, MEL_WINDOW, 32usize, 1usize],
            flat.into_boxed_slice(),
        ))?;
        let outputs = self.emb.run(ort::inputs!["input_1" => input])?;
        let (_shape, data) = outputs["conv2d_19"].try_extract_tensor::<f32>()?;
        let mut emb = [0.0f32; FEAT_DIM];
        let n = data.len().min(FEAT_DIM);
        emb[..n].copy_from_slice(&data[..n]);
        Ok(emb)
    }

    fn run_clf(&mut self) -> Result<f32> {
        let start = self.feat_buf.len() - CLASSIFIER_FRAMES;
        let mut flat = Vec::with_capacity(CLASSIFIER_FRAMES * FEAT_DIM);
        for emb in &self.feat_buf[start..] {
            flat.extend_from_slice(emb);
        }
        let input = Tensor::from_array((
            [1usize, CLASSIFIER_FRAMES, FEAT_DIM],
            flat.into_boxed_slice(),
        ))?;
        let outputs = self.clf.run(ort::inputs!["x.1" => input])?;
        let (_shape, data) = outputs["53"].try_extract_tensor::<f32>()?;
        Ok(data.first().copied().unwrap_or(0.0))
    }
}

/// Localiza libonnxruntime.so y la registra en ORT_DYLIB_PATH si hace falta.
pub fn ensure_onnx_lib() -> Result<PathBuf> {
    // 1. Override explicito
    if let Ok(p) = std::env::var("ORT_DYLIB_PATH") {
        if !p.is_empty() && Path::new(&p).exists() {
            return Ok(PathBuf::from(p));
        }
    }
    // 2. Sistema (ldconfig)
    if let Ok(out) = std::process::Command::new("ldconfig").arg("-p").output() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            if line.contains("libonnxruntime.so")
                && !line.contains("Python")
                && !line.contains("steam")
                && !line.contains("flatpak")
            {
                if let Some(path) = line.split("=>").nth(1) {
                    let p = path.trim().to_string();
                    if Path::new(&p).exists() {
                        std::env::set_var("ORT_DYLIB_PATH", &p);
                        log::info!("ONNX Runtime del sistema: {p}");
                        return Ok(PathBuf::from(p));
                    }
                }
            }
        }
    }
    // 3. Rutas comunes del sistema
    for p in [
        "/usr/lib/libonnxruntime.so",
        "/usr/lib64/libonnxruntime.so",
        "/usr/local/lib/libonnxruntime.so",
    ] {
        if Path::new(p).exists() {
            std::env::set_var("ORT_DYLIB_PATH", p);
            log::info!("ONNX Runtime del sistema: {p}");
            return Ok(PathBuf::from(p));
        }
    }
    // 4. Fallback: copia incluida en el paquete python onnxruntime
    if let Ok(entries) = glob_python_onnx_lib() {
        for p in entries {
            std::env::set_var("ORT_DYLIB_PATH", &p);
            log::info!("ONNX Runtime (fallback paquete python): {}", p.display());
            return Ok(p);
        }
    }
    bail!(
        "libonnxruntime.so no encontrada. Instala con: sudo pacman -S onnxruntime\n\
         o define ORT_DYLIB_PATH con la ruta a libonnxruntime.so"
    )
}

fn glob_python_onnx_lib() -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    // Buscar en site-packages de las versiones de python instaladas
    if let Ok(dir) = std::fs::read_dir("/usr/lib") {
        for entry in dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("python3.") {
                let capi = entry.path().join("site-packages/onnxruntime/capi");
                if let Ok(files) = std::fs::read_dir(&capi) {
                    for f in files.flatten() {
                        let fname = f.file_name().to_string_lossy().to_string();
                        if fname.starts_with("libonnxruntime.so")
                            && !fname.contains("providers_shared")
                        {
                            out.push(f.path());
                        }
                    }
                }
            }
        }
    }
    // Preferir la version sin sufijo, si no la mas reciente
    out.sort_by_key(|p| {
        let s = p.to_string_lossy().to_string();
        (if s.ends_with(".so") { 0 } else { 1 }, std::cmp::Reverse(s))
    });
    Ok(out)
}
