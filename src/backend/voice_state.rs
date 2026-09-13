//! Máquina de estados formal del pipeline de voz (plan de voz §3).
//!
//! El estado se modela con `VoiceState`; la UI sigue consumiendo solo cuatro
//! estados legados (`idle`, `listening`, `processing`, `speaking`) a través
//! de `ui_label()`, así no hay que tocar QML al enriquecer la máquina.
//!
//! Las transiciones siguen el diagrama del plan:
//!
//! ```text
//! IDLE -> WAKE_DETECTED -> ATTENTIVE -> LISTENING -> THINKING -> RESPONDING
//! RESPONDING -> INTERRUPTED -> LISTENING        (barge-in)
//! THINKING -> TOOL_EXECUTING -> THINKING        (ReAct)
//! cualquier estado -> CANCELLED / Error(_) -> IDLE | LISTENING
//! ```

use std::fmt;

/// Tipo de fallo que llevo la máquina a `Error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceErrorKind {
    Audio,
    Stt,
    Llm,
    Tts,
}

/// Estado formal del pipeline de voz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceState {
    Idle,
    WakeDetected,
    Attentive,
    Listening,
    Thinking,
    Responding,
    ToolExecuting,
    Interrupted,
    Cancelled,
    Error(VoiceErrorKind),
}

impl VoiceState {
    /// Nombre canónico para logs estructurados (plan §44).
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::WakeDetected => "wake_detected",
            Self::Attentive => "attentive",
            Self::Listening => "listening",
            Self::Thinking => "thinking",
            Self::Responding => "responding",
            Self::ToolExecuting => "tool_executing",
            Self::Interrupted => "interrupted",
            Self::Cancelled => "cancelled",
            Self::Error(VoiceErrorKind::Audio) => "audio_error",
            Self::Error(VoiceErrorKind::Stt) => "stt_error",
            Self::Error(VoiceErrorKind::Llm) => "llm_error",
            Self::Error(VoiceErrorKind::Tts) => "tts_error",
        }
    }

    /// Estado legado que consume la UI (Main.qml, VoiceOrb, AgentWindow).
    /// La UI filtra cualquier estado fuera de estos cuatro: mapear bien.
    pub fn ui_label(self) -> &'static str {
        match self {
            Self::Idle
            | Self::WakeDetected
            | Self::Attentive
            | Self::Cancelled
            | Self::Error(_) => "idle",
            Self::Listening | Self::Interrupted => "listening",
            Self::Thinking | Self::ToolExecuting => "processing",
            Self::Responding => "speaking",
        }
    }

    /// ¿Transición prevista por el diagrama §3? Informativa: el sistema
    /// aplica igualmente (una transición rara no debe dejar la máquina
    /// clavada), pero queda registrada para auditoría.
    pub fn can_reach(self, next: VoiceState) -> bool {
        use VoiceState::*;
        if self == next {
            return true; // emisión idempotente
        }
        match self {
            Idle => matches!(next, WakeDetected | Listening | Error(_)),
            WakeDetected => matches!(next, Attentive | Listening | Idle | Error(_)),
            Attentive => matches!(next, Listening | Idle | Error(_)),
            Listening => matches!(next, Thinking | Idle | Cancelled | Error(_)),
            Thinking => matches!(
                next,
                Responding | ToolExecuting | Listening | Idle | Cancelled | Interrupted | Error(_)
            ),
            ToolExecuting => matches!(
                next,
                Thinking | Responding | Idle | Cancelled | Interrupted | Error(_)
            ),
            Responding => matches!(next, Interrupted | Listening | Idle | Cancelled | Error(_)),
            Interrupted => matches!(next, Listening | Idle | Cancelled),
            Cancelled => matches!(next, Idle | Listening),
            Error(_) => matches!(next, Idle | Listening | WakeDetected),
        }
    }
}

impl fmt::Display for VoiceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Máquina con estado actual + contador de operaciones (plan §13).
/// Cada turno de voz abre una operación nueva para que los resultados
/// tardíos de una operación cancelada puedan descartarse por id.
#[derive(Debug)]
pub struct VoiceStateMachine {
    state: VoiceState,
    op_id: u64,
}

impl Default for VoiceStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl VoiceStateMachine {
    pub fn new() -> Self {
        Self {
            state: VoiceState::Idle,
            op_id: 0,
        }
    }

    pub fn state(&self) -> VoiceState {
        self.state
    }

    pub fn op_id(&self) -> u64 {
        self.op_id
    }

    /// Abre una operación nueva (turno de voz). Devuelve su id.
    pub fn begin_operation(&mut self) -> u64 {
        self.op_id += 1;
        self.op_id
    }

    /// Transición validada. Las no previstas se aplican igualmente pero
    /// quedan en el log (regla: errores recuperables, nunca clavar la FSM).
    pub fn transition(&mut self, next: VoiceState) -> VoiceState {
        let from = self.state;
        if from == next {
            return next;
        }
        if !from.can_reach(next) {
            log::warn!(
                "[VOICE op={}] transición no prevista: {from} -> {next}",
                self.op_id
            );
        }
        log::info!("[VOICE op={}] {from} -> {next}", self.op_id);
        self.state = next;
        next
    }

    /// Fuerza un estado sin validar (recuperación de errores).
    pub fn force(&mut self, next: VoiceState) {
        log::warn!(
            "[VOICE op={}] forzando estado {} (antes: {})",
            self.op_id,
            next,
            self.state
        );
        self.state = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_mapping_es_compatible_con_qml() {
        // Main.qml descarta estados fuera de estos cuatro (línea ~1444).
        let ui_states = ["idle", "listening", "processing", "speaking"];
        for s in [
            VoiceState::Idle,
            VoiceState::WakeDetected,
            VoiceState::Attentive,
            VoiceState::Listening,
            VoiceState::Thinking,
            VoiceState::Responding,
            VoiceState::ToolExecuting,
            VoiceState::Interrupted,
            VoiceState::Cancelled,
            VoiceState::Error(VoiceErrorKind::Stt),
        ] {
            assert!(
                ui_states.contains(&s.ui_label()),
                "ui_label de {s:?} no lo entiende la UI"
            );
        }
    }

    #[test]
    fn flujo_feliz_es_valido() {
        let mut m = VoiceStateMachine::new();
        m.transition(VoiceState::WakeDetected);
        m.transition(VoiceState::Attentive);
        m.transition(VoiceState::Listening);
        m.transition(VoiceState::Thinking);
        m.transition(VoiceState::ToolExecuting);
        m.transition(VoiceState::Thinking);
        m.transition(VoiceState::Responding);
        m.transition(VoiceState::Idle);
        assert_eq!(m.state(), VoiceState::Idle);
    }

    #[test]
    fn barge_in_es_valido() {
        let mut m = VoiceStateMachine::new();
        m.transition(VoiceState::Listening);
        m.transition(VoiceState::Thinking);
        m.transition(VoiceState::Responding);
        m.transition(VoiceState::Interrupted);
        m.transition(VoiceState::Listening);
        assert_eq!(m.state(), VoiceState::Listening);
    }

    #[test]
    fn transiciones_no_previstas_se_marcan() {
        // Idle -> Responding no está en el diagrama.
        assert!(!VoiceState::Idle.can_reach(VoiceState::Responding));
        assert!(!VoiceState::Listening.can_reach(VoiceState::Responding));
        // Pero la máquina las aplica para no clavarse (y loguea warn).
        let mut m = VoiceStateMachine::new();
        m.transition(VoiceState::Responding);
        assert_eq!(m.state(), VoiceState::Responding);
    }

    #[test]
    fn error_se_recupera_a_idle_o_listening() {
        let e = VoiceState::Error(VoiceErrorKind::Tts);
        assert!(e.can_reach(VoiceState::Idle));
        assert!(e.can_reach(VoiceState::Listening));
        assert!(!e.can_reach(VoiceState::Responding));
    }

    #[test]
    fn operaciones_incrementan_id() {
        let mut m = VoiceStateMachine::new();
        assert_eq!(m.op_id(), 0);
        assert_eq!(m.begin_operation(), 1);
        assert_eq!(m.begin_operation(), 2);
        assert_eq!(m.op_id(), 2);
    }

    #[test]
    fn self_transition_es_noop_valido() {
        assert!(VoiceState::Listening.can_reach(VoiceState::Listening));
        let mut m = VoiceStateMachine::new();
        m.transition(VoiceState::Listening);
        m.transition(VoiceState::Listening); // no debe quejarse ni romper
        assert_eq!(m.state(), VoiceState::Listening);
    }
}
