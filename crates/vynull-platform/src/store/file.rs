use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::{Store, StoreError};

/// All keys in one JSON file, rewritten atomically (temp file + rename) on
/// each change. The file is small, so there is no batching.
#[derive(Debug)]
pub struct FileStore {
    path: PathBuf,
    map: Mutex<BTreeMap<String, String>>,
}

impl FileStore {
    /// `<config dir>/vynull-client/store.json`: `~/.config` on Linux,
    /// `~/Library/Application Support` on macOS, `%APPDATA%` on Windows.
    pub fn default_path() -> Result<PathBuf, StoreError> {
        dirs::config_dir()
            .map(|d| d.join("vynull-client").join("store.json"))
            .ok_or_else(|| StoreError::Unavailable("no config directory".into()))
    }

    pub fn open_default() -> Result<Self, StoreError> {
        Self::open(Self::default_path()?)
    }

    /// Open (or start) the store at `path`. A file that is not valid JSON is
    /// moved aside to `<name>.corrupt` and the store starts empty: this data
    /// is only client preferences, and refusing to start would be worse.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let path = path.into();
        let map = match fs::read_to_string(&path) {
            Ok(raw) => match serde_json::from_str(&raw) {
                Ok(map) => map,
                Err(_) => {
                    let mut aside = path.clone().into_os_string();
                    aside.push(".corrupt");
                    let _ = fs::rename(&path, aside);
                    BTreeMap::new()
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(StoreError::Unavailable(format!("{}: {e}", path.display()))),
        };
        Ok(Self {
            path,
            map: Mutex::new(map),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn save(&self, map: &BTreeMap<String, String>) -> Result<(), StoreError> {
        let werr = |e: io::Error| StoreError::Write(format!("{}: {e}", self.path.display()));
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(werr)?;
        }
        let mut tmp = self.path.clone().into_os_string();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        let json =
            serde_json::to_string_pretty(map).map_err(|e| StoreError::Write(e.to_string()))?;
        fs::write(&tmp, json).map_err(werr)?;
        fs::rename(&tmp, &self.path).map_err(werr)
    }
}

impl Store for FileStore {
    fn get(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self.map.lock().unwrap().get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), StoreError> {
        let mut map = self.map.lock().unwrap();
        map.insert(key.into(), value.into());
        self.save(&map)
    }

    fn remove(&self, key: &str) -> Result<(), StoreError> {
        let mut map = self.map.lock().unwrap();
        if map.remove(key).is_some() {
            self.save(&map)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_across_opens_and_creates_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/store.json");
        let s = FileStore::open(&path).unwrap();
        s.set("theme", "dark").unwrap();
        s.set("volume", "0.5").unwrap();
        s.remove("volume").unwrap();
        drop(s);
        let s = FileStore::open(&path).unwrap();
        assert_eq!(s.get("theme").unwrap().as_deref(), Some("dark"));
        assert_eq!(s.get("volume").unwrap(), None);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn corrupt_file_is_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.json");
        fs::write(&path, "{ not json").unwrap();
        let s = FileStore::open(&path).unwrap();
        assert_eq!(s.get("theme").unwrap(), None);
        assert!(dir.path().join("store.json.corrupt").exists());
        s.set("theme", "light").unwrap();
        assert_eq!(
            FileStore::open(&path)
                .unwrap()
                .get("theme")
                .unwrap()
                .as_deref(),
            Some("light")
        );
    }
}
