use crate::app;
use crate::config::Scripts;
use crate::search::LauncherItem;
use crate::stdin;

pub fn run() {
    let scripts = Scripts::load();
    let items: Vec<LauncherItem> = scripts
        .entries
        .into_iter()
        .map(|(name, entry)| LauncherItem::new(name, entry))
        .collect();
    app::run(items, false, false, None, None);
}

pub fn run_dmenu() {
    let items = stdin::read_stdin().unwrap_or_default();
    app::run(items, true, false, None, None);
}
