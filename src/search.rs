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
    case_sensitive: bool,
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
            case_sensitive,
        }
    }

    pub fn update(&mut self, raw_query: &str) {
        let parsed = ParsedQuery::parse(raw_query);

        let tag_filter_fn = |item: &LauncherItem, tag: &str| -> bool {
            if self.case_sensitive {
                item.entry.tag.iter().any(|t| t.contains(tag))
            } else {
                let tag_lower = tag.to_lowercase();
                item.entry
                    .tag
                    .iter()
                    .any(|t| t.to_lowercase().contains(&tag_lower))
            }
        };
        if parsed.query.is_empty() {
            self.has_query = false;
            self.results = match &parsed.mode {
                QueryMode::Normal => self.all_items.clone(),
                QueryMode::Tag(tag) => {
                    let tag = tag.clone();
                    self.all_items
                        .iter()
                        .filter(|i| tag_filter_fn(i, &tag))
                        .cloned()
                        .collect()
                }
            };
            return;
        }
        if parsed.query.is_empty() {
            self.has_query = false;
            self.results = match &parsed.mode {
                QueryMode::Normal => self.all_items.clone(),
                QueryMode::Tag(tag) => self
                    .all_items
                    .iter()
                    .filter(|i| i.entry.tag.iter().any(|t| t.contains(tag.as_str())))
                    .cloned()
                    .collect(),
            };
            return;
        }

        self.has_query = true;
        let query = if self.case_sensitive {
            parsed.query.clone()
        } else {
            parsed.query.to_lowercase()
        };

        let case_matching = if self.case_sensitive {
            CaseMatching::Respect
        } else {
            CaseMatching::Ignore
        };
        match &parsed.mode {
            QueryMode::Normal => self.run_nucleo(&query, None, case_matching),
            QueryMode::Tag(tag) => {
                let tag = tag.clone();
                self.run_nucleo(&query, Some(tag), case_matching);
            }
        }
    }

    fn run_nucleo(&mut self, query: &str, tag_filter: Option<String>, case_matching: CaseMatching) {
        self.nucleo
            .pattern
            .reparse(0, query, case_matching, Normalization::Smart, false);
        self.nucleo.tick(50);

        let case_sensitive = self.case_sensitive;
        let snapshot = self.nucleo.snapshot();
        self.results = snapshot
            .matched_items(..snapshot.matched_item_count().min(50) as u32)
            .map(|i| i.data.clone())
            .filter(|item| {
                if let Some(ref tag) = tag_filter {
                    if case_sensitive {
                        item.entry.tag.iter().any(|t| t.contains(tag.as_str()))
                    } else {
                        let tag_lower = tag.to_lowercase();
                        item.entry
                            .tag
                            .iter()
                            .any(|t| t.to_lowercase().contains(&tag_lower))
                    }
                } else {
                    true
                }
            })
            .collect();
    }

    pub fn tick(&mut self) {
        if self.has_query && self.nucleo.tick(0).changed {
            let snapshot = self.nucleo.snapshot();
            self.results = snapshot
                .matched_items(..snapshot.matched_item_count().min(50) as u32)
                .map(|i| i.data.clone())
                .collect();
        }
    }
}

// ── Query parser ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum QueryMode {
    Normal,
    Tag(String),
}

#[derive(Debug, Clone)]
pub struct ParsedQuery {
    pub mode: QueryMode,
    pub query: String,
}

impl ParsedQuery {
    pub fn parse(input: &str) -> Self {
        if let Some(rest) = input.strip_prefix('#') {
            let (tag, query) = split_first_word(rest);
            return Self {
                mode: QueryMode::Tag(tag.to_string()),
                query: query.to_string(),
            };
        }

        Self {
            mode: QueryMode::Normal,
            query: input.to_string(),
        }
    }
}

fn split_first_word(s: &str) -> (&str, &str) {
    if let Some(pos) = s.find(' ') {
        (&s[..pos], s[pos + 1..].trim_start())
    } else {
        (s, "")
    }
}
