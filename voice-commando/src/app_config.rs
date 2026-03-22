use vc_common::config::VoiceCommandoConfig;
use std::path::PathBuf;

/// Load the game configuration from disk or use defaults.
pub fn load_config(path: Option<&str>) -> VoiceCommandoConfig {
    let config_path = path
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("config/voice_commando.toml"));

    match VoiceCommandoConfig::load(&config_path) {
        Ok(config) => {
            tracing::info!("Loaded config from {:?}", config_path);
            config
        }
        Err(e) => {
            tracing::warn!("Failed to load config from {:?}: {}. Using defaults.", config_path, e);
            VoiceCommandoConfig::default()
        }
    }
}
