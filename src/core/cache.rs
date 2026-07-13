use std::{collections::HashMap, fmt, sync::Arc, time::Instant};

use async_trait::async_trait;
use tokio::sync::RwLock;

use super::Result;

/// Cached token value and expiry metadata.
#[derive(Clone)]
pub struct CachedToken {
    value: String,
    expires_at: Instant,
}

impl CachedToken {
    /// Creates a cached token that expires at the supplied instant.
    pub fn new(value: impl Into<String>, expires_at: Instant) -> Self {
        Self {
            value: value.into(),
            expires_at,
        }
    }

    /// Returns the token value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the expiry instant.
    pub fn expires_at(&self) -> Instant {
        self.expires_at
    }

    /// Returns `true` when the token is no longer usable.
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
}

impl fmt::Debug for CachedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CachedToken")
            .field("value", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// Pluggable async token cache.
#[async_trait]
pub trait TokenCache: Send + Sync {
    /// Reads a token by key.
    async fn get(&self, key: &str) -> Result<Option<CachedToken>>;
    /// Writes or replaces a token.
    async fn set(&self, key: &str, token: CachedToken) -> Result<()>;
    /// Removes a token.
    async fn remove(&self, key: &str) -> Result<()>;
}

/// In-memory token cache used by default.
#[derive(Clone, Default)]
pub struct MemoryTokenCache {
    values: Arc<RwLock<HashMap<String, CachedToken>>>,
}

impl fmt::Debug for MemoryTokenCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryTokenCache").finish_non_exhaustive()
    }
}

#[async_trait]
impl TokenCache for MemoryTokenCache {
    async fn get(&self, key: &str) -> Result<Option<CachedToken>> {
        let token = self.values.read().await.get(key).cloned();
        if token.as_ref().is_some_and(CachedToken::is_expired) {
            self.values.write().await.remove(key);
            return Ok(None);
        }
        Ok(token)
    }

    async fn set(&self, key: &str, token: CachedToken) -> Result<()> {
        self.values.write().await.insert(key.to_owned(), token);
        Ok(())
    }

    async fn remove(&self, key: &str) -> Result<()> {
        self.values.write().await.remove(key);
        Ok(())
    }
}
