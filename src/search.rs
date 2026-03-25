use crate::config::Entry;
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Nucleo};

#[derive(Clone, Debug)]
pub struct LauncherItem {
    pub name: String,
    pub entry: Entry,
}

impl LauncherItem {
    pub fn new(name: String, entry: Entry) -> Self {
        let display_name = if entry.name.is_empty() {
            name
        } else {
            entry.name.clone()
        };
        Self {
            name: display_name,
            entry,
        }
    }

    pub fn search_text(&self) -> String {
        self.name.clone()
    }
}

pub struct FuzzySearch {
    nucleo: Nucleo<LauncherItem>,
    pub results: Vec<LauncherItem>,
    all_items: Vec<LauncherItem>,
    has_query: bool,
    current_query: String,
    current_tags: Vec<String>,
}

impl FuzzySearch {
    pub fn new(items: Vec<LauncherItem>, case_sensitive: bool) -> Self {
        let nucleo: Nucleo<LauncherItem> =
            Nucleo::new(Config::DEFAULT, std::sync::Arc::new(|| {}), None, 1);

        let injector = nucleo.injector();
        for item in &items {
            let text = if case_sensitive {
                item.search_text()
            } else {
                item.search_text().to_lowercase()
            };
            let owned = item.clone();
            injector.push(owned, |_data, cols| {
                cols[0] = text.into();
            });
        }

        let results = items.clone();

        Self {
            nucleo,
            results,
            all_items: items,
            has_query: false,
            current_query: String::new(),
            current_tags: Vec::new(),
        }
    }

    pub fn update(&mut self, raw_query: &str) {
        let parsed = ParsedQuery::parse(raw_query);

        self.current_query = parsed.text;
        self.current_tags = parsed.tags;

        if self.current_query.is_empty() && self.current_tags.is_empty() {
            self.results = self.all_items.clone();
            self.has_query = false;
            return;
        }

        self.has_query = true;
        self.rebuild_results();
    }

    fn rebuild_results(&mut self) {
        let query = &self.current_query;
        let tags = &self.current_tags;

        self.nucleo
            .pattern
            .reparse(0, query, CaseMatching::Smart, Normalization::Smart, false);
        self.nucleo.tick(50);

        let snapshot = self.nucleo.snapshot();

        let candidates: Vec<LauncherItem> = if query.is_empty() {
            self.all_items.clone()
        } else {
            snapshot
                .matched_items(..snapshot.matched_item_count().min(50) as u32)
                .map(|i| i.data.clone())
                .collect()
        };

        self.results = candidates
            .into_iter()
            .filter(|item| {
                tags.iter().all(|tag| {
                    item.entry
                        .tag
                        .iter()
                        .any(|t| t.to_lowercase().contains(tag.as_str()))
                })
            })
            .collect();
    }

    pub fn tick(&mut self) {
        if self.has_query && self.nucleo.tick(0).changed {
            self.rebuild_results();
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedQuery {
    pub text: String,
    pub tags: Vec<String>,
}

impl ParsedQuery {
    pub fn parse(input: &str) -> Self {
        let mut text_parts = Vec::new();
        let mut tags = Vec::new();
        for token in input.split_whitespace() {
            if let Some(tag) = token.strip_prefix("#") {
                if !tag.is_empty() {
                    tags.push(tag.to_lowercase());
                }
            } else {
                text_parts.push(token);
            }
        }
        Self {
            text: text_parts.join(" "),
            tags,
        }
    }
}
