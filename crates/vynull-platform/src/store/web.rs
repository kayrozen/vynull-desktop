use super::{Store, StoreError};

/// Browser `localStorage`. Keys are prefixed so the client shares an origin
/// safely with other pages (the daemon serves it at `/app` next to `/overlay`).
pub struct WebStore {
    storage: web_sys::Storage,
}

const PREFIX: &str = "vynull.";

impl WebStore {
    /// Fails when storage is blocked (some private modes, disabled cookies).
    /// Fall back to [`super::MemoryStore`] then.
    pub fn new() -> Result<Self, StoreError> {
        let unavailable = |why: &str| StoreError::Unavailable(why.to_string());
        let storage = web_sys::window()
            .ok_or_else(|| unavailable("no window"))?
            .local_storage()
            .map_err(|_| unavailable("localStorage blocked"))?
            .ok_or_else(|| unavailable("no localStorage"))?;
        Ok(Self { storage })
    }
}

fn full(key: &str) -> String {
    format!("{PREFIX}{key}")
}

impl Store for WebStore {
    fn get(&self, key: &str) -> Result<Option<String>, StoreError> {
        self.storage
            .get_item(&full(key))
            .map_err(|e| StoreError::Unavailable(format!("{e:?}")))
    }

    /// Fails when the quota is full.
    fn set(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.storage
            .set_item(&full(key), value)
            .map_err(|e| StoreError::Write(format!("{e:?}")))
    }

    fn remove(&self, key: &str) -> Result<(), StoreError> {
        self.storage
            .remove_item(&full(key))
            .map_err(|e| StoreError::Write(format!("{e:?}")))
    }
}
