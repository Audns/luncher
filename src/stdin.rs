use crate::config::Entry;
use crate::search::LauncherItem;
use std::io::{self, BufRead};

/// Returns Some(items) if stdin is piped, None if stdin is a terminal
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
            LauncherItem::new(
                line.clone(),
                Entry {
                    command: line,
                    tag: vec![],
                },
            )
        })
        .collect::<Vec<_>>();

    Some(items)
}
