//! VAD de energía con endpointing configurable y piso de ruido adaptativo.
//!
//! Plan de voz §6: detectar inicio/fin de turno, tolerar pausas naturales,
//! no cortar demasiado rápido ni esperar de más, y no depender de un timeout
//! fijo. La decisión de corte vive en `VadState` (lógica pura, testeable);
//! `record_until_silence` solo le alimenta RMS por ventanas.
//!
//! Piso adaptativo: percentil 25 de una ventana móvil de los últimos ~5s de
//! RMS. Aprende también en ambientes ruidosos (ventilador, TV): la primera
//! versión solo aprendía de frames < 0.06 y con ruido ambiente alto nunca
//! aprendía → el umbral quedaba en el base → el turno nunca cortaba.

use crate::models::SpeechConfig;

/// Config efectiva del VAD (con defaults si la config no los trae).
#[derive(Debug, Clone, Copy)]
pub struct VadConfig {
    /// Silencio sostenido que corta el turno.
    pub silence_ms: u64,
    /// Mínimo grabado antes de que el silencio pueda cortar.
    pub min_record_ms: u64,
    /// Umbral base de silencio (RMS). Con `adaptive`, es también el piso.
    pub base_rms: f32,
    /// Piso de ruido adaptativo (sube el umbral en ambientes ruidosos).
    pub adaptive: bool,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            silence_ms: 1200,
            min_record_ms: 1500,
            base_rms: 0.02,
            adaptive: true,
        }
    }
}

impl From<&SpeechConfig> for VadConfig {
    fn from(s: &SpeechConfig) -> Self {
        Self {
            silence_ms: s.vad_silence_ms,
            min_record_ms: s.vad_min_record_ms,
            base_rms: s.vad_silence_rms,
            adaptive: s.vad_adaptive,
        }
    }
}

/// Umbral = piso * factor (colchón sobre el ruido de fondo).
const FLOOR_FACTOR: f32 = 2.5;
/// Techo del umbral adaptativo (suficiente para ambientes con TV/música).
const MAX_THRESHOLD: f32 = 0.12;
/// Ventana móvil del estimador de piso (50 polls ≈ 5s a poll de 100ms).
const FLOOR_WINDOW: usize = 50;
/// Mínimo de muestras antes de confiar en el piso estimado.
const MIN_FLOOR_SAMPLES: usize = 5;

/// Resultado de alimentar una ventana al VAD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VadEvent {
    Continue,
    TurnEnded,
}

/// Estado incremental del VAD para un turno de grabación.
#[derive(Debug)]
pub struct VadState {
    cfg: VadConfig,
    floor_hist: std::collections::VecDeque<f32>,
    silent_ms: u64,
}

impl VadState {
    pub fn new(cfg: VadConfig) -> Self {
        Self {
            cfg,
            floor_hist: std::collections::VecDeque::with_capacity(FLOOR_WINDOW),
            silent_ms: 0,
        }
    }

    /// Umbral de silencio vigente (adaptativo si hay piso estimado).
    pub fn silence_threshold(&self) -> f32 {
        match self.estimate_floor() {
            Some(floor) => (floor * FLOOR_FACTOR).clamp(self.cfg.base_rms, MAX_THRESHOLD),
            None => self.cfg.base_rms,
        }
    }

    /// Piso de ruido = percentil 25 de la ventana móvil. El percentil bajo
    /// aísla el fondo incluso con voz frecuente en la ventana.
    fn estimate_floor(&self) -> Option<f32> {
        if !self.cfg.adaptive || self.floor_hist.len() < MIN_FLOOR_SAMPLES {
            return None;
        }
        let mut v: Vec<f32> = self.floor_hist.iter().copied().collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        Some(v[v.len() / 4])
    }

    /// Milisegundos de silencio acumulados actualmente.
    pub fn silent_ms(&self) -> u64 {
        self.silent_ms
    }

    /// Alimenta una ventana RMS de ~`poll_ms`.
    /// `recorded_ms` = audio grabado hasta ahora (puede diferir del tiempo
    /// transcurrido si el mic todavía no entrega frames).
    /// El silencio solo cuenta hacia el corte una vez superado
    /// `min_record_ms` (igual que el pipeline histórico: 1.5s + 1.2s).
    pub fn push(&mut self, rms: f32, recorded_ms: u64, poll_ms: u64) -> VadEvent {
        let threshold = self.silence_threshold();
        let past_min = recorded_ms >= self.cfg.min_record_ms;
        if past_min {
            if rms < threshold {
                self.silent_ms = self.silent_ms.saturating_add(poll_ms);
            } else {
                self.silent_ms = 0;
            }
        }

        // El piso se aprende de TODOS los frames (la ventana móvil con p25
        // ya filtra la voz); no se descarta nada por ser "demasiado alto".
        if self.cfg.adaptive {
            self.floor_hist.push_back(rms);
            if self.floor_hist.len() > FLOOR_WINDOW {
                self.floor_hist.pop_front();
            }
        }

        if past_min && self.silent_ms >= self.cfg.silence_ms {
            VadEvent::TurnEnded
        } else {
            VadEvent::Continue
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> VadConfig {
        VadConfig {
            adaptive: false,
            ..VadConfig::default()
        }
    }

    #[test]
    fn silencio_sostenido_corta_el_turno() {
        let mut vad = VadState::new(cfg());
        // 2s grabados, 1.2s de silencio sostenido en ventanas de 100ms.
        let mut end_ms = None;
        for i in 1..=400u64 {
            if vad.push(0.005, i * 10, 10) == VadEvent::TurnEnded {
                end_ms = Some(i * 10);
                break;
            }
        }
        let t = end_ms.expect("debió cortar por silencio");
        // min 1500ms grabados + 1200ms de silencio ≈ 2690-2700ms (la
        // primera ventana de silencio cuenta en el mismo poll del mínimo).
        assert!((2680..=2710).contains(&t), "cortó en {t}ms");
    }

    #[test]
    fn voz_resetea_el_silencio() {
        let mut vad = VadState::new(cfg());
        for i in 1..60u64 {
            // Voz cada 1s (huecos de 1s < 1.2s de gracia): nunca corta.
            let rms = if i % 10 == 9 { 0.2 } else { 0.005 };
            assert_eq!(vad.push(rms, i * 100, 100), VadEvent::Continue);
        }
        // Ahora dejar silencio puro hasta que corte.
        let mut ended = false;
        for i in 60..80u64 {
            if vad.push(0.005, i * 100, 100) == VadEvent::TurnEnded {
                ended = true;
                break;
            }
        }
        assert!(ended, "debió cortar tras 1.2s de silencio sostenido");
    }

    #[test]
    fn no_corta_antes_del_minimo_grabado() {
        let mut vad = VadState::new(VadConfig {
            min_record_ms: 3000,
            ..cfg()
        });
        // 2.9s grabados en silencio total: no puede cortar aún.
        for i in 1..=290u64 {
            assert_eq!(vad.push(0.0, i * 10, 10), VadEvent::Continue);
        }
        // Superado el mínimo + colgado de silencio, corta.
        let mut ended = false;
        for i in 291..600u64 {
            if vad.push(0.0, i * 10, 10) == VadEvent::TurnEnded {
                ended = true;
                break;
            }
        }
        assert!(ended);
    }

    #[test]
    fn piso_adaptativo_sube_el_umbral_en_ruido() {
        let mut vad = VadState::new(VadConfig::default()); // adaptive = true
                                                           // Ambiente con ruido constante ~0.04 (ventilador, tele).
                                                           // Mientras no llega al mínimo grabado (1.5s), jamás corta.
        for i in 1..10u64 {
            assert_eq!(vad.push(0.04, i * 100, 100), VadEvent::Continue);
        }
        let thr = vad.silence_threshold();
        assert!(
            thr > 0.02,
            "el umbral debió subir por encima del base (0.02), es {thr}"
        );
        assert!(thr <= 0.12, "el umbral no debe pasar el techo, es {thr}");
        // Con el umbral adaptado, el ruido de fondo (0.04 < thr) cuenta como
        // silencio y el turno puede cerrarse.
        let mut ended = false;
        for i in 10..60u64 {
            if vad.push(0.04, i * 100, 100) == VadEvent::TurnEnded {
                ended = true;
                break;
            }
        }
        assert!(
            ended,
            "con umbral adaptado el ruido de fondo no colgó el turno"
        );
    }

    #[test]
    fn ambiente_ruidoso_aptaumbra_y_corta() {
        // Regresión real (2026-09-13): ambiente ruidoso con p50=0.15 hacía
        // que el piso nunca se aprendiera (la regla vieja exigía rms<0.06) y
        // el turno siempre terminaba por timeout de 8s.
        let mut vad = VadState::new(VadConfig::default());
        // 3s de ambiente ruidoso, luego discurso claro, luego solo ambiente.
        for i in 1..=30u64 {
            vad.push(0.10, i * 100, 100);
        }
        // El umbral sube por encima del ambiente.
        let thr = vad.silence_threshold();
        assert!(
            thr > 0.10,
            "umbral {thr} debe superar el ambiente (0.10) o el turno nunca corta"
        );
        // Discurso alto → no cuenta como silencio.
        for i in 31..=45u64 {
            assert_eq!(vad.push(0.30, i * 100, 100), VadEvent::Continue);
        }
        // El usuario para de hablar: queda el ambiente (0.10 < thr) y debe cortar.
        let mut ended = false;
        for i in 46..=80u64 {
            if vad.push(0.10, i * 100, 100) == VadEvent::TurnEnded {
                ended = true;
                break;
            }
        }
        assert!(
            ended,
            "el turno debió cortar en ambiente ruidoso tras el discurso"
        );
    }

    #[test]
    fn sin_adaptativo_el_umbral_es_siempre_el_base() {
        let mut vad = VadState::new(VadConfig {
            adaptive: false,
            ..VadConfig::default()
        });
        for i in 1..50u64 {
            vad.push(0.04, i * 100, 100);
        }
        assert!((vad.silence_threshold() - 0.02).abs() < f32::EPSILON);
    }

    #[test]
    fn piso_bajo_no_baja_el_umbral_del_default() {
        let mut vad = VadState::new(VadConfig::default());
        // Ambiente casi digitalmente silencioso.
        for i in 1..50u64 {
            vad.push(0.001, i * 100, 100);
        }
        // El clamp mantiene al menos el base_rms: evita que ruido lejano
        // mantenga el turno abierto en una habitación muy silenciosa.
        assert!((vad.silence_threshold() - 0.02).abs() < f32::EPSILON);
    }
}
