use crate::config::Entry;
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Nucleo};

#[derive(Clone, Debug)]
pub struct LauncherItem {
    pub name: String,
    pub entry: Entry,
    pub search_text: String,
}

impl LauncherItem {
    pub fn new(name: String, entry: Entry) -> Self {
        let mut search_text = name.clone();
        for tag in &entry.tag {
            search_text.push(' ');
            search_text.push_str(tag);
        }
        Self {
            name,
            entry,
            search_text,
        }
    }
}

pub struct FuzzySearch {
    nucleo: Nucleo<LauncherItem>,
    pub results: Vec<LauncherItem>,
}

impl FuzzySearch {
    pub fn new(items: Vec<LauncherItem>) -> Self {
        let nucleo = Nucleo::new(Config::DEFAULT, std::sync::Arc::new(|| {}), None, 1);

        let injector = nucleo.injector();
        for item in items {
            let search_text = item.search_text.clone();
            injector.push(item, |_data, cols| {
                cols[0] = search_text.into();
            });
        }

        Self {
            nucleo,
            results: Vec::new(),
        }
    }

    pub fn update(&mut self, query: &str) {
        self.nucleo
            .pattern
            .reparse(0, query, CaseMatching::Smart, Normalization::Smart, false);
        self.nucleo.tick(50);
        self.collect_results();
    }

    pub fn tick(&mut self) {
        if self.nucleo.tick(0).changed {
            self.collect_results();
        }
    }

    fn collect_results(&mut self) {
        let snapshot = self.nucleo.snapshot();
        self.results = snapshot
            .matched_items(..snapshot.matched_item_count().min(50) as u32)
            .map(|item| item.data.clone())
            .collect();
    }
}
