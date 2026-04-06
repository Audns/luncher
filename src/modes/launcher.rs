use crate::app;
use crate::clipboard::client;
use crate::config::Entry;
use crate::search::LauncherItem;
use freedesktop_desktop_entry::{Iter, default_paths, get_languages_from_env};

pub fn run(rt: tokio::runtime::Runtime) {
    rt.handle().spawn(async {
        let _ = client::ensure_daemon().await;
    });
    app::run(
        Vec::new(),
        false,
        false,
        Some(app::RemoteSource::Launcher),
        Some(rt.handle().clone()),
        Some(rt),
        "Launcher".to_string(),
    );
}

pub fn load_items() -> Vec<LauncherItem> {
    let locales = get_languages_from_env();

    let mut items: Vec<LauncherItem> = Iter::new(default_paths())
        .entries(Some(&locales))
        .filter_map(|entry| {
            if entry.no_display() {
                return None;
            }
            if entry.hidden() {
                return None;
            }
            if entry.type_()? != "Application" {
                return None;
            }

            let name = entry.name(&locales)?.to_string();
            if name.is_empty() {
                return None;
            }

            let exec = entry.exec().unwrap_or("").to_string();
            let command = format_command(&strip_field_codes(&exec), entry.terminal());

            let categories: Vec<String> = entry
                .categories()
                .unwrap_or_default()
                .iter()
                .flat_map(|s| s.split(';'))
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();

            Some(LauncherItem {
                name: name.clone(),
                entry: Entry {
                    name: String::new(),
                    command,
                    tag: categories,
                    inline_meta: None,
                },
            })
        })
        .collect();

    items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    items
}

fn strip_field_codes(exec: &str) -> String {
    let mut out = String::with_capacity(exec.len());
    let mut chars = exec.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            chars.next();
        } else {
            out.push(ch);
        }
    }
    out.trim().to_string()
}

fn format_command(cmd: &str, terminal: bool) -> String {
    if terminal {
        for term in &["foot", "kitty", "alacritty", "wezterm", "xterm"] {
            if which(term) {
                return format!("{} -e {}", term, cmd);
            }
        }
    }
    cmd.to_string()
}

fn which(bin: &str) -> bool {
    std::process::Command::new("which")
        .arg(bin)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
