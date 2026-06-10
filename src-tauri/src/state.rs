use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AiState {
    #[serde(rename = "working")]
    Working,
    #[serde(rename = "stopped")]
    Stopped,
    #[serde(rename = "warning")]
    Warning,
}

impl AiState {
    pub fn as_str(&self) -> &'static str {
        match self {
            AiState::Working => "working",
            AiState::Stopped => "stopped",
            AiState::Warning => "warning",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            AiState::Working => "#4CAF50",
            AiState::Stopped => "#9E9E9E",
            AiState::Warning => "#FFC107",
        }
    }

    pub fn flashing(&self) -> bool {
        matches!(self, AiState::Warning)
    }
}

pub struct AppState {
    pub current: AiState,
    pub monitor_directory: Option<String>,
    pub last_change_time: Option<Instant>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            current: AiState::Stopped,
            monitor_directory: None,
            last_change_time: None,
        }
    }
}

pub type SharedState = Arc<RwLock<AppState>>;

pub fn create_shared_state() -> SharedState {
    Arc::new(RwLock::new(AppState::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_stopped() {
        let state = AppState::new();
        assert_eq!(state.current, AiState::Stopped);
        assert!(state.monitor_directory.is_none());
        assert!(state.last_change_time.is_none());
    }

    #[test]
    fn test_working_properties() {
        assert_eq!(AiState::Working.as_str(), "working");
        assert_eq!(AiState::Working.color(), "#4CAF50");
        assert!(!AiState::Working.flashing());
    }

    #[test]
    fn test_stopped_properties() {
        assert_eq!(AiState::Stopped.as_str(), "stopped");
        assert_eq!(AiState::Stopped.color(), "#9E9E9E");
        assert!(!AiState::Stopped.flashing());
    }

    #[test]
    fn test_warning_properties() {
        assert_eq!(AiState::Warning.as_str(), "warning");
        assert_eq!(AiState::Warning.color(), "#FFC107");
        assert!(AiState::Warning.flashing());
    }

    #[test]
    fn test_serde_roundtrip() {
        for state in &[AiState::Working, AiState::Stopped, AiState::Warning] {
            let json = serde_json::to_string(state).unwrap();
            let deserialized: AiState = serde_json::from_str(&json).unwrap();
            assert_eq!(*state, deserialized);
        }
    }

    #[test]
    fn test_create_shared_state() {
        let shared = create_shared_state();
        let state = shared.read().unwrap();
        assert_eq!(state.current, AiState::Stopped);
    }
}
