//! Retired F1 hotword adapter. Production wake detection belongs to VoiceCoordinator.
//! Kept as an inert capability probe for old startup callers; no energy/ZCR fallback.
use crate::models::Config;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct HotwordDetector;
impl HotwordDetector {
    pub async fn new(_config: Arc<RwLock<Config>>) -> Result<Self> {
        Ok(Self)
    }
    pub fn ml_active(&self) -> bool {
        false
    }
    pub fn is_running(&self) -> bool {
        false
    }
}
