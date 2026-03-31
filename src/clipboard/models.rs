use bytes::Bytes;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
    Text,
    Image,
    Binary,
    Sensitive,
}

impl EntryKind {
    pub fn from_mime(mime: &str, sensitive: bool) -> Self {
        if sensitive {
            return Self::Sensitive;
        }
        if mime.starts_with("text/") || matches!(mime, "UTF8_STRING" | "STRING" | "TEXT") {
            Self::Text
        } else if mime.starts_with("image/") {
            Self::Image
        } else {
            Self::Binary
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub id: u64,
    pub timestamp: u64,
    pub mime_type: String,
    pub kind: EntryKind,
    #[serde(with = "bytes_serde")]
    pub data: Bytes,
    #[serde(with = "bytes_serde")]
    pub thumb: Bytes,
    pub hash: [u8; 32],
    pub sensitive: bool,
    pub filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LegacyClipboardEntryV1 {
    pub id: u64,
    pub timestamp: u64,
    pub mime_type: String,
    #[serde(with = "bytes_serde")]
    pub data: Bytes,
    pub hash: [u8; 32],
    pub sensitive: bool,
}

impl ClipboardEntry {
    pub fn with_filename(
        mime_type: impl Into<String>,
        data: Bytes,
        sensitive: bool,
        thumb: Bytes,
        filename: Option<String>,
    ) -> Self {
        let mime_type = mime_type.into();
        let now = now_micros();
        let kind = EntryKind::from_mime(&mime_type, sensitive);

        let hash = {
            let mut h = blake3::Hasher::new();
            h.update(mime_type.as_bytes());
            h.update(&data);
            *h.finalize().as_bytes()
        };

        Self {
            id: now,
            timestamp: now,
            mime_type,
            kind,
            data,
            thumb,
            hash,
            sensitive,
            filename,
        }
    }

    pub fn preview(&self, max_chars: usize) -> String {
        if let Some(name) = &self.filename {
            return match self.kind {
                EntryKind::Sensitive => "********".to_string(),
                EntryKind::Image => format!("{} · {} KB", name, self.data.len() / 1024),
                EntryKind::Text if self.mime_type == "text/uri-list" => format!("{} · path", name),
                EntryKind::Text => format!("{} · text", name),
                EntryKind::Binary => format!("{} · {}", name, self.mime_type),
            };
        }

        match self.kind {
            EntryKind::Sensitive => "********".to_string(),
            EntryKind::Text => {
                let s = String::from_utf8_lossy(&self.data);
                let flat: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
                let mut out: String = flat.chars().take(max_chars).collect();
                if flat.chars().count() > max_chars {
                    out.push('…');
                }
                out
            }
            EntryKind::Image => format!(
                "image/{} · {} KB",
                self.mime_type.strip_prefix("image/").unwrap_or("?"),
                self.data.len() / 1024,
            ),
            EntryKind::Binary => format!("{} · {} B", self.mime_type, self.data.len()),
        }
    }

    pub fn full_content(&self) -> String {
        if let Some(name) = &self.filename {
            return match self.kind {
                EntryKind::Sensitive => "********".to_string(),
                EntryKind::Image => format!("{} · {} KB", name, self.data.len() / 1024),
                EntryKind::Text if self.mime_type == "text/uri-list" => format!("{} · path", name),
                EntryKind::Text => format!("{} · text", name),
                EntryKind::Binary => format!("{} · {}", name, self.mime_type),
            };
        }

        match self.kind {
            EntryKind::Sensitive => "********".to_string(),
            EntryKind::Text => String::from_utf8_lossy(&self.data).to_string(),
            EntryKind::Image => format!(
                "image/{} · {} KB",
                self.mime_type.strip_prefix("image/").unwrap_or("?"),
                self.data.len() / 1024,
            ),
            EntryKind::Binary => format!("{} · {} B", self.mime_type, self.data.len()),
        }
    }

    pub(crate) fn from_stored_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        match postcard::from_bytes(bytes) {
            Ok(entry) => Ok(entry),
            Err(_) => {
                let legacy: LegacyClipboardEntryV1 = postcard::from_bytes(bytes)?;
                Ok(Self {
                    id: legacy.id,
                    timestamp: legacy.timestamp,
                    kind: EntryKind::from_mime(&legacy.mime_type, legacy.sensitive),
                    mime_type: legacy.mime_type,
                    data: legacy.data,
                    thumb: Bytes::new(),
                    hash: legacy.hash,
                    sensitive: legacy.sensitive,
                    filename: None,
                })
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryMeta {
    pub id: u64,
    pub timestamp: u64,
    pub mime_type: String,
    pub kind: EntryKind,
    pub data_len: usize,
    pub sensitive: bool,
    pub preview: String,
    pub thumb: Vec<u8>,
    pub filename: Option<String>,
}

impl From<&ClipboardEntry> for EntryMeta {
    fn from(entry: &ClipboardEntry) -> Self {
        Self {
            id: entry.id,
            timestamp: entry.timestamp,
            mime_type: entry.mime_type.clone(),
            kind: entry.kind,
            data_len: entry.data.len(),
            sensitive: entry.sensitive,
            preview: entry.preview(120),
            thumb: entry.thumb.to_vec(),
            filename: entry.filename.clone(),
        }
    }
}

fn now_micros() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

pub(crate) mod bytes_serde {
    use bytes::Bytes;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(bytes: &Bytes, serializer: S) -> Result<S::Ok, S::Error> {
        bytes.as_ref().serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Bytes, D::Error> {
        Ok(Bytes::from(Vec::<u8>::deserialize(deserializer)?))
    }
}
