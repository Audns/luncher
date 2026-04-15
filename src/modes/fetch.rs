use crate::config::Scripts;
use crate::modes::launcher;
use crate::search::{FuzzySearch, LauncherItem};
use serde_json::json;

pub fn run(pattern: &str, case_sensitive: bool, only_script: bool, only_launcher: bool) -> String {
    let script_entries = Scripts::load().entries;
    let launcher_items = launcher::load_items();

    let script_items: Vec<LauncherItem> = script_entries
        .into_iter()
        .map(|(name, entry)| LauncherItem::new(name, entry))
        .collect();

    let launcher_items: Vec<LauncherItem> = launcher_items;

    let (script_results, launcher_results) = if only_script {
        let mut search = FuzzySearch::new(script_items.clone(), case_sensitive);
        search.update(&pattern);
        (search.results, vec![])
    } else if only_launcher {
        let mut search = FuzzySearch::new(launcher_items.clone(), case_sensitive);
        search.update(&pattern);
        (vec![], search.results)
    } else {
        let mut all_items: Vec<LauncherItem> = script_items.clone();
        all_items.extend(launcher_items.clone());
        let mut search = FuzzySearch::new(all_items, case_sensitive);
        search.update(&pattern);

        let mut script_map = serde_json::Map::new();
        let mut launcher_map = serde_json::Map::new();

        for item in search.results {
            if item.entry.name.is_empty() {
                script_map.insert(
                    item.name.clone(),
                    json!({
                        "command": item.entry.command,
                        "hashtag": item.entry.tag.join(" ")
                    }),
                );
            } else {
                launcher_map.insert(
                    item.name.clone(),
                    json!({
                        "command": item.entry.command,
                        "hashtag": item.entry.tag.join(" ")
                    }),
                );
            }
        }

        return serde_json::to_string_pretty(&json!({
            "script": script_map,
            "launcher": launcher_map
        }))
        .unwrap_or_default();
    };

    let mut script_map = serde_json::Map::new();
    let mut launcher_map = serde_json::Map::new();

    for item in script_results {
        script_map.insert(
            item.name,
            json!({
                "command": item.entry.command,
                "hashtag": item.entry.tag.join(" ")
            }),
        );
    }

    for item in launcher_results {
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
