use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::SystemTime;

use crate::error::StorageError;

/// Bounded on-disk + in-memory LRU cache for cover artwork bytes.
#[derive(Debug)]
pub struct ArtCache {
    cache_dir: PathBuf,
    max_disk_bytes: u64,
    max_mem_entries: usize,
    mem_entries: HashMap<String, Vec<u8>>,
    mem_lru: VecDeque<String>,
    disk_sizes: HashMap<String, u64>,
    disk_lru: VecDeque<String>,
    total_disk_bytes: u64,
}

impl ArtCache {
    pub fn default_cache_dir() -> PathBuf {
        let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join("LumeMusic").join("cache").join("art")
    }

    pub fn new(cache_dir: PathBuf, max_disk_bytes: u64, max_mem_entries: usize) -> Self {
        let _ = std::fs::create_dir_all(&cache_dir);
        let mut cache = Self {
            cache_dir,
            max_disk_bytes,
            max_mem_entries,
            mem_entries: HashMap::new(),
            mem_lru: VecDeque::new(),
            disk_sizes: HashMap::new(),
            disk_lru: VecDeque::new(),
            total_disk_bytes: 0,
        };
        cache.hydrate_from_disk();
        cache
    }

    pub fn default_disk_budget() -> u64 {
        100 * 1024 * 1024
    }

    pub fn default_mem_entries() -> usize {
        30
    }

    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    fn key_filename(key: &str) -> String {
        let mut hash: u64 = 0xcbf29ce484222325;
        for &b in key.as_bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        format!("{hash:016x}.art")
    }

    fn path_for_filename(&self, file_name: &str) -> PathBuf {
        self.cache_dir.join(file_name)
    }

    fn hydrate_from_disk(&mut self) {
        let Ok(entries) = std::fs::read_dir(&self.cache_dir) else {
            return;
        };
        let mut files: Vec<(String, u64, SystemTime)> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("art") {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                files.push((stem.to_string(), meta.len(), mtime));
            }
        }
        files.sort_by_key(|(_, _, t)| *t);
        for (stem, len, _) in files {
            let disk_id = format!("__file__:{stem}");
            self.disk_sizes.insert(disk_id.clone(), len);
            self.disk_lru.push_back(disk_id);
            self.total_disk_bytes += len;
        }
        let _ = self.evict_if_needed(None);
    }

    fn disk_id_for_key(key: &str) -> String {
        let stem = Self::key_filename(key);
        let stem = stem.trim_end_matches(".art");
        format!("__file__:{stem}")
    }

    fn file_path_for_disk_id(&self, disk_id: &str) -> PathBuf {
        let stem = disk_id.strip_prefix("__file__:").unwrap_or(disk_id);
        self.path_for_filename(&format!("{stem}.art"))
    }

    pub fn art_path(&self, key: &str) -> Option<PathBuf> {
        let path = self.path_for_filename(&Self::key_filename(key));
        path.exists().then_some(path)
    }

    fn touch_mem(&mut self, key: &str, bytes: Vec<u8>) {
        if self.max_mem_entries == 0 {
            return;
        }
        self.mem_lru.retain(|k| k != key);
        self.mem_lru.push_back(key.to_string());
        self.mem_entries.insert(key.to_string(), bytes);
        while self.mem_entries.len() > self.max_mem_entries {
            if let Some(oldest) = self.mem_lru.pop_front() {
                self.mem_entries.remove(&oldest);
            } else {
                break;
            }
        }
    }

    fn touch_disk(&mut self, disk_id: &str) {
        self.disk_lru.retain(|k| k != disk_id);
        self.disk_lru.push_back(disk_id.to_string());
    }

    pub fn get(&mut self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        if let Some(bytes) = self.mem_entries.get(key).cloned() {
            self.touch_mem(key, bytes.clone());
            let disk_id = Self::disk_id_for_key(key);
            if self.disk_sizes.contains_key(&disk_id) {
                self.touch_disk(&disk_id);
            }
            return Ok(Some(bytes));
        }

        let path = self.path_for_filename(&Self::key_filename(key));
        if !path.exists() {
            return Ok(None);
        }

        let bytes = std::fs::read(&path)?;
        let disk_id = Self::disk_id_for_key(key);
        if !self.disk_sizes.contains_key(&disk_id) {
            let len = bytes.len() as u64;
            self.disk_sizes.insert(disk_id.clone(), len);
            self.total_disk_bytes += len;
        }
        self.touch_disk(&disk_id);
        self.touch_mem(key, bytes.clone());
        Ok(Some(bytes))
    }

    pub fn put(&mut self, key: &str, bytes: &[u8]) -> Result<PathBuf, StorageError> {
        std::fs::create_dir_all(&self.cache_dir)?;
        let path = self.path_for_filename(&Self::key_filename(key));
        let disk_id = Self::disk_id_for_key(key);

        if let Some(prev_size) = self.disk_sizes.remove(&disk_id) {
            self.total_disk_bytes = self.total_disk_bytes.saturating_sub(prev_size);
        }

        std::fs::write(&path, bytes)?;
        let len = bytes.len() as u64;
        self.disk_sizes.insert(disk_id.clone(), len);
        self.total_disk_bytes += len;
        self.touch_disk(&disk_id);
        self.touch_mem(key, bytes.to_vec());
        self.evict_if_needed(Some(&disk_id))?;

        Ok(path)
    }

    pub fn evict_if_needed(&mut self, protected_key: Option<&str>) -> Result<(), StorageError> {
        while self.total_disk_bytes > self.max_disk_bytes {
            let victim_idx = self
                .disk_lru
                .iter()
                .position(|k| Some(k.as_str()) != protected_key);
            let Some(idx) = victim_idx else {
                break;
            };
            let Some(victim_id) = self.disk_lru.remove(idx) else {
                break;
            };
            if let Some(size) = self.disk_sizes.remove(&victim_id) {
                self.total_disk_bytes = self.total_disk_bytes.saturating_sub(size);
            }
            let path = self.file_path_for_disk_id(&victim_id);
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
        }
        Ok(())
    }

    pub fn total_disk_bytes(&self) -> u64 {
        self.total_disk_bytes
    }

    pub fn mem_len(&self) -> usize {
        self.mem_entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn art_cache_evicts_lru_files_when_exceeding_byte_budget() {
        let dir = TempDir::new().unwrap();
        let mut cache = ArtCache::new(dir.path().to_path_buf(), 250, 2);

        let a = vec![1u8; 100];
        let b = vec![2u8; 100];
        let c = vec![3u8; 100];

        let path_a = cache.put("track:a", &a).unwrap();
        let path_b = cache.put("track:b", &b).unwrap();
        // Touch track:a so track:b becomes least recently used.
        assert_eq!(cache.get("track:a").unwrap(), Some(a.clone()));

        let path_c = cache.put("track:c", &c).unwrap();
        assert!(cache.total_disk_bytes() <= 250);
        assert!(path_a.exists());
        assert!(!path_b.exists());
        assert!(path_c.exists());
        assert!(cache.mem_len() <= 2);
    }
}
