use std::process::Command;

use serde::Deserialize;

use crate::app;
use crate::config::Entry;
use crate::search::LauncherItem;

#[derive(Debug, Deserialize)]
struct HyprWorkspace {
    id: i32,
}

#[derive(Debug, Deserialize)]
struct HyprClient {
    workspace: HyprWorkspace,
    #[serde(default)]
    title: String,
    #[serde(default)]
    class: String,
}

pub fn run() {
    let items = load_items();
    if items.is_empty() {
        eprintln!("[switcher] no Hyprland windows found");
        return;
    }

    app::run(items, false, false, None, None, None);
}

fn load_items() -> Vec<LauncherItem> {
    let output = match Command::new("hyprctl").args(["clients", "-j"]).output() {
        Ok(output) => output,
        Err(err) => {
            eprintln!("[switcher] failed to run hyprctl: {err}");
            return Vec::new();
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("[switcher] hyprctl clients -j failed: {}", stderr.trim());
        return Vec::new();
    }

    let mut clients: Vec<HyprClient> = match serde_json::from_slice(&output.stdout) {
        Ok(clients) => clients,
        Err(err) => {
            eprintln!("[switcher] failed to parse hyprctl output: {err}");
            return Vec::new();
        }
    };

    clients.sort_by(|a, b| {
        a.workspace
            .id
            .cmp(&b.workspace.id)
            .then_with(|| simplified_class_tag(&a.class).cmp(&simplified_class_tag(&b.class)))
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });

    clients
        .into_iter()
        .map(|client| {
            let label = format_label(&client.class, &client.title, client.workspace.id);
            let command = format!("hyprctl dispatch workspace {}", client.workspace.id);
            LauncherItem::new(
                label,
                Entry {
                    name: String::new(),
                    command,
                    tag: build_tags(client.workspace.id, &client.class),
                    inline_meta: Some(String::new()),
                },
            )
        })
        .collect()
}

fn format_label(class: &str, title: &str, workspace_id: i32) -> String {
    let simple_class = simplify_class_label(class);
    let title = title.trim();

    let prefix = format!("{workspace_id}: ");

    match (simple_class.is_empty(), title.is_empty()) {
        (true, true) => format!("{prefix}<unnamed window>"),
        (false, true) => format!("{prefix}{simple_class}"),
        (true, false) => format!("{prefix}{title}"),
        (false, false) => format!("{prefix}{simple_class} - {title}"),
    }
}

fn simplified_class_tag(class: &str) -> String {
    simplify_class_label(class).to_lowercase()
}

fn build_tags(workspace_id: i32, class: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let class = class.trim();
    if !class.is_empty() {
        tags.push(class.to_string());
    }
    tags.push(workspace_id.to_string());
    tags
}

fn simplify_class_label(class: &str) -> String {
    let trimmed = class.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    trimmed
        .rsplit(['.', '-'])
        .next()
        .unwrap_or(trimmed)
        .trim()
        .to_string()
}
