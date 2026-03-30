use crate::config::Entry;
use crate::search::LauncherItem;
use std::io::{self, BufRead};

pub fn read_stdin() -> Option<Vec<LauncherItem>> {
    if atty::is(atty::Stream::Stdin) {
        return None;
    }

    let stdin = io::stdin();
    let items = stdin
        .lock()
        .lines()
        .filter_map(|l| l.ok())
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let display = if let Some(tab_pos) = line.find('\t') {
                line[tab_pos + 1..].to_string()
            } else {
                line.clone()
            };
            LauncherItem::new(
                display,
                Entry {
                    name: line.clone(),
                    command: line,
                    tag: vec![],
                    inline_meta: None,
                },
            )
        })
        .collect::<Vec<_>>();

    Some(items)
}
