use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use super::{AppType, CachedToken, Config, Error, Result, TokenCache};

const APP_ACCESS_TOKEN_INTERNAL_PATH: &str = "/open-apis/auth/v3/app_access_token/internal";
const APP_ACCESS_TOKEN_MARKETPLACE_PATH: &str = "/open-apis/auth/v3/app_access_token";
const TENANT_ACCESS_TOKEN_INTERNAL_PATH: &str = "/open-apis/auth/v3/tenant_access_token/internal";
const TENANT_ACCESS_TOKEN_MARKETPLACE_PATH: &str = "/open-apis/auth/v3/tenant_access_token";
const EXPIRY_SAFETY_WINDOW_SECONDS: u64 = 180;

#[derive(Clone)]
pub(crate) struct TokenManager {
    config: Arc<Config>,
    http: reqwest::Client,
    cache: Arc<dyn TokenCache>,
}

impl TokenManager {
    pub(crate) fn new(
        config: Arc<Config>,
        http: reqwest::Client,
        cache: Arc<dyn TokenCache>,
    ) -> Self {
        Self {
            config,
            http,
            cache,
        }
    }

    pub(crate) async fn app_access_token(&self, app_ticket: Option<&str>) -> Result<String> {
        self.ensure_app_secret()?;
        let key = self.app_cache_key();
        if let Some(token) = self.cached(&key).await? {
            return Ok(token);
        }

        let response = match self.config.app_type {
            AppType::SelfBuilt => {
                self.post_token(
                    APP_ACCESS_TOKEN_INTERNAL_PATH,
                    &SelfBuiltTokenRequest {
                        app_id: &self.config.app_id,
                        app_secret: &self.config.app_secret,
                    },
                )
                .await?
            }
            AppType::Marketplace => {
                let app_ticket = app_ticket
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        Error::InvalidParameter(
                            "app_ticket is required for marketplace applications".into(),
                        )
                    })?;
                self.post_token(
                    APP_ACCESS_TOKEN_MARKETPLACE_PATH,
                    &MarketplaceAppTokenRequest {
                        app_id: &self.config.app_id,
                        app_secret: &self.config.app_secret,
                        app_ticket,
                    },
                )
                .await?
            }
        };

        let token = response
            .app_access_token
            .filter(|value| !value.is_empty())
            .ok_or_else(|| Error::InvalidResponse("app_access_token is missing".into()))?;
        self.store(&key, &token, response.expire).await?;
        Ok(token)
    }

    pub(crate) async fn tenant_access_token(
        &self,
        tenant_key: Option<&str>,
        app_ticket: Option<&str>,
    ) -> Result<String> {
        self.ensure_app_secret()?;
        if self.config.app_type == AppType::Marketplace
            && tenant_key.is_none_or(|value| value.trim().is_empty())
        {
            return Err(Error::InvalidParameter(
                "tenant_key is required for marketplace applications".into(),
            ));
        }

        let key = self.tenant_cache_key(tenant_key.unwrap_or_default());
        if let Some(token) = self.cached(&key).await? {
            return Ok(token);
        }

        let response = match self.config.app_type {
            AppType::SelfBuilt => {
                self.post_token(
                    TENANT_ACCESS_TOKEN_INTERNAL_PATH,
                    &SelfBuiltTokenRequest {
                        app_id: &self.config.app_id,
                        app_secret: &self.config.app_secret,
                    },
                )
                .await?
            }
            AppType::Marketplace => {
                let app_access_token = self.app_access_token(app_ticket).await?;
                self.post_token(
                    TENANT_ACCESS_TOKEN_MARKETPLACE_PATH,
                    &MarketplaceTenantTokenRequest {
                        app_access_token: &app_access_token,
                        tenant_key: tenant_key.unwrap_or_default(),
                    },
                )
                .await?
            }
        };

        let token = response
            .tenant_access_token
            .filter(|value| !value.is_empty())
            .ok_or_else(|| Error::InvalidResponse("tenant_access_token is missing".into()))?;
        self.store(&key, &token, response.expire).await?;
        Ok(token)
    }

    pub(crate) async fn invalidate_app(&self) -> Result<()> {
        self.cache.remove(&self.app_cache_key()).await
    }

    pub(crate) async fn invalidate_tenant(&self, tenant_key: Option<&str>) -> Result<()> {
        self.cache
            .remove(&self.tenant_cache_key(tenant_key.unwrap_or_default()))
            .await
    }

    async fn cached(&self, key: &str) -> Result<Option<String>> {
        if !self.config.enable_token_cache {
            return Ok(None);
        }
        Ok(self
            .cache
            .get(key)
            .await?
            .filter(|token| !token.is_expired())
            .map(|token| token.value().to_owned()))
    }

    async fn store(&self, key: &str, token: &str, expires_in: u64) -> Result<()> {
        if !self.config.enable_token_cache {
            return Ok(());
        }
        let ttl = expires_in
            .saturating_sub(EXPIRY_SAFETY_WINDOW_SECONDS)
            .max(1);
        self.cache
            .set(
                key,
                CachedToken::new(token, Instant::now() + Duration::from_secs(ttl)),
            )
            .await
    }

    async fn post_token<T: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<TokenResponse> {
        let url = self.config.base_url.join(path.trim_start_matches('/'))?;
        let response = self.http.post(url).json(body).send().await?;
        let status = response.status();
        let headers = response.headers().clone();
        let body_text = response.text().await?;
        let request_id = request_id(&headers);

        if body_text.trim().is_empty() {
            return Err(Error::InvalidResponse("empty token response".into()));
        }

        let decoded: TokenResponse = serde_json::from_str(&body_text)?;
        if decoded.code != 0 {
            return Err(Error::Api {
                code: decoded.code,
                message: if decoded.msg.is_empty() {
                    "token request failed".into()
                } else {
                    decoded.msg.clone()
                },
                request_id,
            });
        }
        if !status.is_success() {
            return Err(Error::HttpStatus {
                status,
                body: truncate(&body_text),
            });
        }
        Ok(decoded)
    }

    fn ensure_app_secret(&self) -> Result<()> {
        if self.config.app_secret.trim().is_empty() {
            return Err(Error::InvalidParameter(
                "app_secret is required for automatic app or tenant token acquisition".into(),
            ));
        }
        Ok(())
    }

    fn app_cache_key(&self) -> String {
        format!("app_access_token:{}", self.config.app_id)
    }

    fn tenant_cache_key(&self, tenant_key: &str) -> String {
        format!(
            "tenant_access_token:{}:{}:{}",
            match self.config.app_type {
                AppType::SelfBuilt => "self_built",
                AppType::Marketplace => "marketplace",
            },
            self.config.app_id,
            tenant_key
        )
    }
}

#[derive(Serialize)]
struct SelfBuiltTokenRequest<'a> {
    app_id: &'a str,
    app_secret: &'a str,
}

#[derive(Serialize)]
struct MarketplaceAppTokenRequest<'a> {
    app_id: &'a str,
    app_secret: &'a str,
    app_ticket: &'a str,
}

#[derive(Serialize)]
struct MarketplaceTenantTokenRequest<'a> {
    app_access_token: &'a str,
    tenant_key: &'a str,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    #[serde(default)]
    code: i64,
    #[serde(default)]
    msg: String,
    #[serde(default)]
    expire: u64,
    app_access_token: Option<String>,
    tenant_access_token: Option<String>,
}

fn request_id(headers: &http::HeaderMap) -> Option<String> {
    ["x-tt-logid", "x-request-id", "request-id"]
        .into_iter()
        .find_map(|name| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned)
        })
}

fn truncate(value: &str) -> String {
    const MAX_CHARS: usize = 2_048;
    if value.chars().count() <= MAX_CHARS {
        value.to_owned()
    } else {
        format!("{}…", value.chars().take(MAX_CHARS).collect::<String>())
    }
}
