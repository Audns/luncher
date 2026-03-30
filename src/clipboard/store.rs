use std::path::Path;

use anyhow::{Context, Result};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};

use crate::clipboard::models::ClipboardEntry;

const ENTRIES: TableDefinition<u64, &[u8]> = TableDefinition::new("entries");
const HASHES: TableDefinition<&[u8], u64> = TableDefinition::new("hashes");
const MAX_ENTRIES: usize = 1_000;
const MAX_BYTES: u64 = 64 * 1024 * 1024;

pub struct Store {
    db: Database,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating store dir {}", parent.display()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
            }
        }

        let db = match Database::create(path) {
            Ok(db) => db,
            Err(e) => {
                // Check if this is a v2 format database that needs migration
                if path.exists() {
                    tracing::warn!("Failed to open database, attempting migration: {}", e);
                    // Backup the old database
                    let backup_path = path.with_extension("redb.bak");
                    std::fs::rename(path, &backup_path).with_context(|| {
                        format!("backing up database to {}", backup_path.display())
                    })?;
                    tracing::info!("Backed up old database to {}", backup_path.display());
                    // Create a new database
                    Database::create(path)
                        .with_context(|| format!("creating new redb at {}", path.display()))?
                } else {
                    return Err(e).with_context(|| format!("opening redb at {}", path.display()));
                }
            }
        };

        let tx = db.begin_write()?;
        tx.open_table(ENTRIES)?;
        tx.open_table(HASHES)?;
        tx.commit()?;

        Ok(Self { db })
    }

    pub fn insert(&self, entry: &ClipboardEntry) -> Result<Option<u64>> {
        let tx = self.db.begin_write()?;

        let is_duplicate = {
            let hash_table = tx.open_table(HASHES)?;
            hash_table.get(entry.hash.as_ref())?.is_some()
        };
        if is_duplicate {
            tx.abort()?;
            return Ok(None);
        }

        let serialized = postcard::to_allocvec(entry)?;

        {
            let mut entries = tx.open_table(ENTRIES)?;
            let mut hashes = tx.open_table(HASHES)?;
            entries.insert(entry.id, serialized.as_slice())?;
            hashes.insert(entry.hash.as_ref(), entry.id)?;
        }

        tx.commit()?;
        self.evict_if_needed()?;
        Ok(Some(entry.id))
    }

    pub fn get_recent(&self, limit: usize) -> Result<Vec<ClipboardEntry>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(ENTRIES)?;

        let mut entries = Vec::with_capacity(limit);
        for result in table.iter()?.rev() {
            let (_, value) = result?;
            if let Ok(entry) = ClipboardEntry::from_stored_bytes(value.value()) {
                entries.push(entry);
            }
            if entries.len() == limit {
                break;
            }
        }

        Ok(entries)
    }

    pub fn get_by_id(&self, id: u64) -> Result<Option<ClipboardEntry>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(ENTRIES)?;

        match table.get(id)? {
            Some(value) => Ok(Some(ClipboardEntry::from_stored_bytes(value.value())?)),
            None => Ok(None),
        }
    }

    pub fn delete(&self, id: u64) -> Result<bool> {
        let tx = self.db.begin_write()?;
        let removed;

        {
            let mut entries = tx.open_table(ENTRIES)?;
            let mut hashes = tx.open_table(HASHES)?;
            if let Some(raw) = entries.remove(id)? {
                if let Ok(entry) = ClipboardEntry::from_stored_bytes(raw.value()) {
                    hashes.remove(entry.hash.as_ref())?;
                }
                removed = true;
            } else {
                removed = false;
            }
        }

        tx.commit()?;
        Ok(removed)
    }

    fn evict_if_needed(&self) -> Result<()> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(ENTRIES)?;

        let entries: Vec<(u64, u64)> = table
            .iter()?
            .map(|result| result.map(|(key, value)| (key.value(), value.value().len() as u64)))
            .collect::<Result<_, _>>()?;

        let count = entries.len();
        let mut total_bytes: u64 = entries.iter().map(|(_, len)| *len).sum();
        if count <= MAX_ENTRIES && total_bytes <= MAX_BYTES {
            return Ok(());
        }

        let mut remaining = count;
        let mut ids_to_remove = Vec::new();
        for (id, len) in entries {
            if remaining <= MAX_ENTRIES && total_bytes <= MAX_BYTES {
                break;
            }
            ids_to_remove.push(id);
            remaining = remaining.saturating_sub(1);
            total_bytes = total_bytes.saturating_sub(len);
        }

        drop(table);
        drop(tx);

        for id in ids_to_remove {
            self.delete(id)?;
        }

        Ok(())
    }
}

pub type SharedStore = std::sync::Arc<Store>;
