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

    #[serde(default)]
    pub theme: ThemeConfig,

    #[serde(default)]
    pub layout: LayoutConfig,

    #[serde(default)]
    pub clipboard: ClipboardConfig,
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

#[derive(Debug, Deserialize)]
pub struct ThemeConfig {
    #[serde(default = "default_bg")]
    pub bg: u32,
    #[serde(default = "default_fg")]
    pub fg: u32,
    #[serde(default = "default_fg_dim")]
    pub fg_dim: u32,
    #[serde(default = "default_fg_hint")]
    pub fg_hint: u32,
    #[serde(default = "default_sel_bg")]
    pub sel_bg: u32,
    #[serde(default = "default_line")]
    pub line: u32,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            bg: default_bg(),
            fg: default_fg(),
            fg_dim: default_fg_dim(),
            fg_hint: default_fg_hint(),
            sel_bg: default_sel_bg(),
            line: default_line(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LayoutConfig {
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_hint_size")]
    pub hint_size: f32,
    #[serde(default = "default_row_h")]
    pub row_h: u32,
    #[serde(default = "default_input_h")]
    pub input_h: u32,
    #[serde(default = "default_pad_x")]
    pub pad_x: u32,
    #[serde(default = "default_input_letter_spacing")]
    pub input_letter_spacing: f32,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            font_size: default_font_size(),
            hint_size: default_hint_size(),
            row_h: default_row_h(),
            input_h: default_input_h(),
            pad_x: default_pad_x(),
            input_letter_spacing: default_input_letter_spacing(),
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
            theme: ThemeConfig::default(),
            layout: LayoutConfig::default(),
            clipboard: ClipboardConfig::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ClipboardConfig {
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            history_limit: default_history_limit(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub tag: Vec<String>,
    #[serde(default)]
    pub inline_meta: Option<String>,
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

fn default_history_limit() -> usize {
    50
}

fn default_font_size() -> f32 {
    22.0
}
fn default_hint_size() -> f32 {
    22.0
}
fn default_row_h() -> u32 {
    58
}
fn default_input_h() -> u32 {
    45
}
fn default_pad_x() -> u32 {
    16
}
fn default_input_letter_spacing() -> f32 {
    0.5
}

fn default_bg() -> u32 {
    0xFF1E1E2E
}
fn default_fg() -> u32 {
    0xE6E6E6FF
}
fn default_fg_dim() -> u32 {
    0x73C0CAF5
}
fn default_fg_hint() -> u32 {
    0x4DC0CAF5
}
fn default_sel_bg() -> u32 {
    0x15C0CAF5
}
fn default_line() -> u32 {
    0xFF2A2A3E
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("luncher")
}
