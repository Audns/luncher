use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_scale")]
    pub scale: f32,

    #[serde(default = "default_true")]
    pub single_instance: bool,

    #[serde(default = "default_false")]
    pub case_sensitive: bool,

    #[serde(default)]
    pub window: WindowConfig,
}

#[derive(Debug, Deserialize)]
pub struct WindowConfig {
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: default_width(),
            height: default_height(),
        }
    }
}

fn default_scale() -> f32 {
    1.0
}
fn default_width() -> u32 {
    1200
}
fn default_height() -> u32 {
    800
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}

impl Config {
    pub fn load() -> Self {
        let path = config_dir().join("config.toml");
        let text = fs::read_to_string(&path).unwrap_or_default();
        toml::from_str(&text).unwrap_or_else(|e| {
            eprintln!("[config] parse error: {e}");
            Self::default()
        })
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scale: default_scale(),
            window: WindowConfig::default(),
            single_instance: default_true(),
            case_sensitive: default_false(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub tag: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Scripts {
    #[serde(flatten)]
    pub entries: IndexMap<String, Entry>,
}

impl Scripts {
    pub fn load() -> Self {
        let path = config_dir().join("scripts.toml");
        let text = fs::read_to_string(&path).unwrap_or_default();
        toml::from_str(&text).unwrap_or_else(|e| {
            eprintln!("[scripts] parse error: {e}");
            Self {
                entries: IndexMap::new(),
            }
        })
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("luncher")
}
