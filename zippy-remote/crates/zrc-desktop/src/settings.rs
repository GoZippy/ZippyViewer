use serde::{Deserialize, Serialize};
// use std::path::PathBuf;
use directories::ProjectDirs;

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub theme: Theme,
    pub scale_factor: f32,
    pub input_sensitivity: f32,
    pub default_input_mode: String, // "view_only" or "control"
    pub rendezvous_url: String,
    pub relay_urls: Vec<String>,
    pub connection_timeout_secs: u32,
    pub font_size: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            scale_factor: 1.0,
            input_sensitivity: 1.0,
            default_input_mode: "view_only".to_string(),
            rendezvous_url: "https://zrc.dev/api".to_string(),
            relay_urls: Vec::new(),
            connection_timeout_secs: 30,
            font_size: 14.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Theme {
    System,
    Light,
    Dark,
}

impl Settings {
    pub fn load() -> Self {
        if let Some(proj_dirs) = ProjectDirs::from("com", "zippy", "zrc-desktop") {
            let config_path = proj_dirs.config_dir().join("settings.json");
            if config_path.exists() {
                if let Ok(file) = std::fs::File::open(config_path) {
                    if let Ok(settings) = serde_json::from_reader(file) {
                        return settings;
                    }
                }
            }
        }
        Self::default()
    }
    
    pub fn save(&self) {
        if let Some(proj_dirs) = ProjectDirs::from("com", "zippy", "zrc-desktop") {
            let config_dir = proj_dirs.config_dir();
            if std::fs::create_dir_all(config_dir).is_ok() {
                let config_path = config_dir.join("settings.json");
                if let Ok(file) = std::fs::File::create(config_path) {
                    let _ = serde_json::to_writer_pretty(file, self);
                }
            }
        }
    }
}
