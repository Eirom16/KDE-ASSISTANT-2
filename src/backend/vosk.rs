//! Native C API in a disposable restricted worker. Never loaded by Backend/Qt.
//! Only the exact runtime evaluated in F4-R2 is accepted, before dlopen.
use anyhow::{Context, Result};
use libloading::Library;
use sha2::{Digest, Sha256};
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    path::Path,
    ptr::NonNull,
};

const LIBRARY_BYTES: u64 = 25_986_496;
const LIBRARY_SHA256: &str = "85c4654de3acdeb99abab86eeb2a6e603927d37089597c0fcc33d8638dc2ccaf";
const MAX_PCM: usize = 120 * 16000 * 2;
const MAX_TEXT: usize = 32768;
const MODEL_FILES: &[(&str, u64, &str)] = &[
    (
        "am/final.mdl",
        16098256,
        "ff9508e4db01d2e2375da3887973f94ef8057ee85411f4cb10a5c174ce7a4425",
    ),
    (
        "conf/mfcc.conf",
        155,
        "e3c6cae9faa5da8a5a952cede8729af15166f325bb512789ec1ad6c0fd99e97f",
    ),
    (
        "conf/model.conf",
        289,
        "330c1b50575d0c1c0402276c9cdc7a589ebd04de5d3de17695dd7e6db701da54",
    ),
    (
        "graph/Gr.fst",
        21818215,
        "0ddc8aacbc80b2e8d470060a429eb26706b2ae1d32615a499befd60cc86a0304",
    ),
    (
        "graph/HCLr.fst",
        12226498,
        "ce42fba42b99076f16bfe2892c3cab0936fb7409edba95c545be81c74f2f59e5",
    ),
    (
        "graph/disambig_tid.int",
        50,
        "de29a2e1ae927c7207d93e93687e25242f7dcdcc9d7246b092364427aca54c24",
    ),
    (
        "graph/phones/word_boundary.int",
        1131,
        "581390c44961af08d79afc2b36ac41289b219810c85c049ddf5c6de0442ea92a",
    ),
    (
        "ivector/final.dubm",
        168048,
        "71b6830ae8cbef8764ccc9a6c65943a0e80f28b32982ae4e32f2d1ff15fadf14",
    ),
    (
        "ivector/final.ie",
        9927287,
        "601057024fc8292ed8c8bd82686f254c390868024b20c4978dfc37d82529ba05",
    ),
    (
        "ivector/final.mat",
        44975,
        "81a445f1589de2251562879efd0ca55d2954d937d4ae5f653aa3f11e6fdea5ef",
    ),
    (
        "ivector/global_cmvn.stats",
        1081,
        "489e52a6f34fdf3dac2293784e6e096803c5b5387fe8ad9fbdcd03a65be8917c",
    ),
    (
        "ivector/online_cmvn.conf",
        95,
        "a2f3571754b64297cb7efb2e7ca3df61995c5a45fcbb97188f90613552bb2dfe",
    ),
    (
        "ivector/splice.conf",
        35,
        "9f0c5f7c82d18eaf25d8bce470efa9f7741f88411fe428774bc0a9bb69a24756",
    ),
];

fn verified_library(path: &Path) -> Result<File> {
    anyhow::ensure!(path.is_absolute(), "Ruta absoluta de libvosk requerida");
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(path)
        .context("Biblioteca Vosk no instalada o no accesible")?;
    let info = file.metadata()?;
    anyhow::ensure!(
        info.is_file() && info.len() == LIBRARY_BYTES,
        "Runtime Vosk no compatible; se requiere 0.3.45 x86_64 evaluado"
    );
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut input = (&mut file).take(LIBRARY_BYTES + 1);
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    anyhow::ensure!(
        hex::encode(digest.finalize()) == LIBRARY_SHA256,
        "Integridad de libvosk inválida; no se ejecutó la biblioteca"
    );
    Ok(file)
}

pub fn validate_paths(library: &Path, model: &Path) -> Result<()> {
    let _verified = verified_library(library)?;
    anyhow::ensure!(
        model.is_absolute() && model.is_dir() && !model.is_symlink(),
        "Modelo Vosk small español no instalado (directorio absoluto requerido)"
    );
    for (relative, size, expected_hash) in MODEL_FILES {
        let file = model.join(relative);
        let info = std::fs::symlink_metadata(&file).context("Modelo Vosk español incompleto")?;
        anyhow::ensure!(
            info.is_file() && info.len() == *size,
            "Archivo de modelo Vosk ausente, corrupto o versión no evaluada"
        );
        let mut input = OpenOptions::new()
            .read(true)
            .custom_flags(
                (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
            )
            .open(&file)?
            .take(size + 1);
        let mut digest = Sha256::new();
        let mut buffer = [0u8; 65536];
        loop {
            let count = input.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            digest.update(&buffer[..count]);
        }
        anyhow::ensure!(
            hex::encode(digest.finalize()) == *expected_hash,
            "Integridad del modelo Vosk small español 0.42 inválida"
        );
    }
    Ok(())
}

type NewModel = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type Free = unsafe extern "C" fn(*mut c_void);
type NewRecognizer = unsafe extern "C" fn(*mut c_void, f32) -> *mut c_void;
type Accept = unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> c_int;
type Text = unsafe extern "C" fn(*mut c_void) -> *const c_char;

struct Api {
    model_new: NewModel,
    model_free: Free,
    recognizer_new: NewRecognizer,
    recognizer_free: Free,
    accept: Accept,
    result: Text,
    final_result: Text,
    _library: Library,
    _file: File,
}

impl Api {
    fn load(path: &Path) -> Result<Self> {
        let file = verified_library(path)?;
        // Load the verified open inode, avoiding a path replacement between hash
        // and dlopen. This is not a sandbox against other processes of this UID.
        let fd_path = format!("/proc/self/fd/{}", file.as_raw_fd());
        // SAFETY: the pinned official runtime exports these signatures (vosk_api.h).
        // All pointers are confined to this worker; the Library outlives their use.
        unsafe {
            let library = Library::new(fd_path).context("No se pudo cargar libvosk local")?;
            let log: unsafe extern "C" fn(c_int) = *library.get(b"vosk_set_log_level\0")?;
            log(-1);
            Ok(Self {
                model_new: *library.get(b"vosk_model_new\0")?,
                model_free: *library.get(b"vosk_model_free\0")?,
                recognizer_new: *library.get(b"vosk_recognizer_new\0")?,
                recognizer_free: *library.get(b"vosk_recognizer_free\0")?,
                accept: *library.get(b"vosk_recognizer_accept_waveform\0")?,
                result: *library.get(b"vosk_recognizer_result\0")?,
                final_result: *library.get(b"vosk_recognizer_final_result\0")?,
                _library: library,
                _file: file,
            })
        }
    }
}

struct Model<'a> {
    api: &'a Api,
    ptr: NonNull<c_void>,
}
impl Drop for Model<'_> {
    fn drop(&mut self) {
        // SAFETY: exactly one owner, library alive, recognizers already dropped.
        unsafe { (self.api.model_free)(self.ptr.as_ptr()) }
    }
}
struct Recognizer<'a> {
    model: &'a Model<'a>,
    ptr: NonNull<c_void>,
}
impl Drop for Recognizer<'_> {
    fn drop(&mut self) {
        // SAFETY: exactly one owner and the borrowed model/API remain alive.
        unsafe { (self.model.api.recognizer_free)(self.ptr.as_ptr()) }
    }
}

fn append_text(output: &mut String, ptr: *const c_char) -> Result<()> {
    anyhow::ensure!(!ptr.is_null(), "Vosk no devolvió resultado");
    // SAFETY: Vosk owns this NUL-terminated result until the next recognizer call.
    let bytes = unsafe { CStr::from_ptr(ptr) }.to_bytes();
    anyhow::ensure!(bytes.len() <= 65536, "Resultado Vosk demasiado grande");
    let value: serde_json::Value =
        serde_json::from_slice(bytes).context("Respuesta Vosk inválida")?;
    let text = value
        .get("text")
        .and_then(|v| v.as_str())
        .context("Resultado Vosk sin texto")?
        .trim();
    anyhow::ensure!(
        output.len() + text.len() < MAX_TEXT,
        "Transcripción demasiado grande"
    );
    if !text.is_empty() {
        if !output.is_empty() {
            output.push(' ');
        }
        output.push_str(text);
    }
    Ok(())
}

pub fn transcribe(library: &Path, model: &Path, pcm: &[u8]) -> Result<String> {
    transcribe_with_observer(library, model, pcm, || Ok(()))
}

pub fn transcribe_with_observer(
    library: &Path,
    model: &Path,
    pcm: &[u8],
    before_inference: impl FnOnce() -> Result<()>,
) -> Result<String> {
    anyhow::ensure!(
        pcm.len() <= MAX_PCM && pcm.len().is_multiple_of(2),
        "PCM Vosk inválido o demasiado largo"
    );
    validate_paths(library, model)?;
    let api = Api::load(library)?;
    let model_name = CString::new(model.to_str().context("Ruta de modelo no UTF-8")?)?;
    // SAFETY: CString and API alive; null failures are rejected before use.
    let ptr = NonNull::new(unsafe { (api.model_new)(model_name.as_ptr()) })
        .context("Modelo Vosk ausente o corrupto")?;
    let model = Model { api: &api, ptr };
    // SAFETY: model alive and PCM sample rate fixed by the capture contract.
    let ptr = NonNull::new(unsafe { (api.recognizer_new)(model.ptr.as_ptr(), 16000.0) })
        .context("No se pudo preparar Vosk")?;
    let recognizer = Recognizer { model: &model, ptr };
    before_inference()?;
    let mut text = String::new();
    for chunk in pcm.chunks(6400) {
        // SAFETY: byte slice is valid for the call, <=6400 bytes, PCM16LE mono.
        let accepted = unsafe {
            (api.accept)(
                recognizer.ptr.as_ptr(),
                chunk.as_ptr().cast(),
                chunk.len() as c_int,
            )
        };
        anyhow::ensure!(accepted >= 0, "Falló la inferencia Vosk");
        if accepted != 0 {
            // SAFETY: recognizer lives until after result extraction.
            append_text(&mut text, unsafe { (api.result)(recognizer.ptr.as_ptr()) })?;
        }
    }
    // SAFETY: same recognizer ownership and result lifetime as above.
    append_text(&mut text, unsafe {
        (api.final_result)(recognizer.ptr.as_ptr())
    })?;
    Ok(text)
}

pub fn worker(library: &Path, model: &Path) -> Result<()> {
    let mut pcm = Vec::new();
    std::io::stdin()
        .take(MAX_PCM as u64 + 1)
        .read_to_end(&mut pcm)?;
    let text = transcribe(library, model, &pcm)?;
    std::io::stdout().write_all(&serde_json::to_vec(&text)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_runtime_invalid_pcm_and_relative_paths_fail_before_native_load() {
        assert!(validate_paths(Path::new("relative"), Path::new("relative")).is_err());
        assert!(validate_paths(
            Path::new("/nonexistent/f4r2.so"),
            Path::new("/nonexistent/model")
        )
        .is_err());
        assert!(transcribe(Path::new("relative"), Path::new("relative"), &[0]).is_err());
        assert!(transcribe(
            Path::new("relative"),
            Path::new("relative"),
            &vec![0; MAX_PCM + 2]
        )
        .is_err());
    }
    #[test]
    fn arbitrary_shared_library_is_rejected_without_dlopen() {
        let path = std::env::temp_dir().join(format!("f4r2-lib-{}", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"not an authorized library").unwrap();
        assert!(verified_library(&path).is_err());
        let file = OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(LIBRARY_BYTES).unwrap();
        assert!(verified_library(&path)
            .unwrap_err()
            .to_string()
            .contains("Integridad"));
        std::fs::remove_file(path).unwrap();
    }
}
