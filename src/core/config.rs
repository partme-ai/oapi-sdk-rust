use std::{fmt, time::Duration};

use http::HeaderMap;
use url::Url;

/// Default Feishu Open Platform API base URL.
pub const FEISHU_BASE_URL: &str = "https://open.feishu.cn";
/// Default Lark Open Platform API base URL.
pub const LARK_BASE_URL: &str = "https://open.larksuite.com";

/// Application distribution type.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AppType {
    /// A custom application created inside one tenant.
    #[default]
    SelfBuilt,
    /// An application distributed through the marketplace.
    Marketplace,
}

/// Immutable client configuration.
#[derive(Clone)]
pub struct Config {
    pub(crate) app_id: String,
    pub(crate) app_secret: String,
    pub(crate) base_url: Url,
    pub(crate) oauth_base_url: Option<Url>,
    pub(crate) timeout: Duration,
    pub(crate) app_type: AppType,
    pub(crate) enable_token_cache: bool,
    pub(crate) default_headers: HeaderMap,
    pub(crate) source: Option<String>,
}

impl Config {
    /// Application ID used by this client.
    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    /// API base URL.
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// OAuth base URL when explicitly configured.
    pub fn oauth_base_url(&self) -> Option<&Url> {
        self.oauth_base_url.as_ref()
    }

    /// Per-request timeout.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Application type.
    pub fn app_type(&self) -> AppType {
        self.app_type
    }

    /// Whether automatic token caching is enabled.
    pub fn token_cache_enabled(&self) -> bool {
        self.enable_token_cache
    }

    pub(crate) fn user_agent(&self) -> String {
        let base = format!("lark-oapi-rust/{}", env!("CARGO_PKG_VERSION"));
        match self.source.as_deref() {
            Some(source) if !source.trim().is_empty() => format!("{base} {source}"),
            _ => base,
        }
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("app_id", &self.app_id)
            .field("app_secret", &"<redacted>")
            .field("base_url", &self.base_url)
            .field("oauth_base_url", &self.oauth_base_url)
            .field("timeout", &self.timeout)
            .field("app_type", &self.app_type)
            .field("enable_token_cache", &self.enable_token_cache)
            .field("default_headers", &self.default_headers)
            .field("source", &self.source)
            .finish()
    }
}
