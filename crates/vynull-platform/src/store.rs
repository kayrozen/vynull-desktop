//! Client-side persistence for data the API cannot hold: search history,
//! column layout, color history, waveform colors, theme, row density, volume
//! and metronome state. Values are strings; [`StoreExt`] adds JSON helpers.
//!
//! The trait is synchronous (localStorage is, and the native file is small) and
//! has no `Send`/`Sync` bound, because the browser handle has none. Share it
//! with `Rc` or `Arc`.

use std::cell::RefCell;
use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("storage unavailable: {0}")]
    Unavailable(String),
    #[error("write failed: {0}")]
    Write(String),
    #[error("stored value for `{key}` is not valid: {source}")]
    Json {
        key: String,
        source: serde_json::Error,
    },
}

/// A string key-value store.
pub trait Store {
    fn get(&self, key: &str) -> Result<Option<String>, StoreError>;
    fn set(&self, key: &str, value: &str) -> Result<(), StoreError>;
    fn remove(&self, key: &str) -> Result<(), StoreError>;
}

/// JSON helpers for any [`Store`], including `dyn Store`.
pub trait StoreExt: Store {
    /// `None` if the key is absent. A value that no longer parses (for example
    /// after a type change between releases) is an error; callers usually
    /// fall back to the default with `unwrap_or_default()` on the result.
    fn get_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, StoreError> {
        match self.get(key)? {
            None => Ok(None),
            Some(raw) => serde_json::from_str(&raw)
                .map(Some)
                .map_err(|source| StoreError::Json {
                    key: key.to_string(),
                    source,
                }),
        }
    }

    fn set_json<T: Serialize + ?Sized>(&self, key: &str, value: &T) -> Result<(), StoreError> {
        let raw = serde_json::to_string(value).map_err(|e| StoreError::Write(e.to_string()))?;
        self.set(key, &raw)
    }
}

impl<S: Store + ?Sized> StoreExt for S {}

/// In-memory store for tests and as a fallback when real storage is blocked.
#[derive(Debug, Default)]
pub struct MemoryStore(RefCell<BTreeMap<String, String>>);

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Store for MemoryStore {
    fn get(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self.0.borrow().get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.0.borrow_mut().insert(key.into(), value.into());
        Ok(())
    }

    fn remove(&self, key: &str) -> Result<(), StoreError> {
        self.0.borrow_mut().remove(key);
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod file;
#[cfg(not(target_arch = "wasm32"))]
pub use file::FileStore;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::WebStore;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Prefs {
        density: String,
        volume: f32,
    }

    #[test]
    fn json_round_trip_through_dyn_store() {
        let s: Box<dyn Store> = Box::new(MemoryStore::new());
        assert_eq!(s.get_json::<Prefs>("prefs").unwrap(), None);
        let p = Prefs {
            density: "compact".into(),
            volume: 0.5,
        };
        s.set_json("prefs", &p).unwrap();
        assert_eq!(s.get_json::<Prefs>("prefs").unwrap(), Some(p));
        s.remove("prefs").unwrap();
        assert_eq!(s.get("prefs").unwrap(), None);
    }

    #[test]
    fn stale_value_is_an_error_not_a_panic() {
        let s = MemoryStore::new();
        s.set("prefs", "{\"density\":1}").unwrap();
        assert!(matches!(
            s.get_json::<Prefs>("prefs"),
            Err(StoreError::Json { .. })
        ));
        assert_eq!(s.get_json::<Prefs>("prefs").unwrap_or_default(), None);
    }
}
