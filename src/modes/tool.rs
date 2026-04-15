use crate::config::Scripts;
use crate::modes::launcher;
use serde_json::json;

pub fn run_json() -> String {
    let script_entries = Scripts::load().entries;
    let launcher_items = launcher::load_items();

    let mut script_map = serde_json::Map::new();
    for (name, entry) in script_entries {
        script_map.insert(
            name,
            json!({
                "command": entry.command,
                "hashtag": entry.tag.join(" ")
            }),
        );
    }

    let mut launcher_map = serde_json::Map::new();
    for item in launcher_items {
        launcher_map.insert(
            item.name,
            json!({
                "command": item.entry.command,
                "hashtag": item.entry.tag.join(" ")
            }),
        );
    }

    let output = json!({
        "script": script_map,
        "launcher": launcher_map
    });

    serde_json::to_string_pretty(&output).unwrap_or_default()
}
