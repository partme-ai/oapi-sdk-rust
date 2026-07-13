//! One-click Feishu/Lark application registration.
//!
//! This module implements the device-code registration flow used by the
//! official Java, Go and Python SDKs. It can pre-fill an application name,
//! description and avatar candidates, and can attach incremental scopes,
//! events and callbacks to the confirmation page.

use std::{fmt, io::Write, time::Duration};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use flate2::{Compression, GzBuilder};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time::{sleep, Instant};
use url::Url;

const SDK_NAME: &str = "rust-sdk";
const DEFAULT_FEISHU_DOMAIN: &str = "https://accounts.feishu.cn";
const DEFAULT_LARK_DOMAIN: &str = "https://accounts.larksuite.com";
const ENDPOINT: &str = "/oauth/v1/app/registration";
const DEFAULT_POLL_INTERVAL_SECONDS: u64 = 5;
const DEFAULT_EXPIRE_SECONDS: u64 = 600;
const AVATAR_MAX_COUNT: usize = 6;

/// Errors returned by one-click registration.
#[derive(Debug, Error)]
pub enum RegistrationError {
    /// Invalid caller input.
    #[error("registration: invalid argument: {0}")]
    InvalidArgument(String),
    /// Network request failed.
    #[error("registration: network error: {0}")]
    Network(#[from] reqwest::Error),
    /// URL parsing or construction failed.
    #[error("registration: invalid URL: {0}")]
    Url(#[from] url::ParseError),
    /// JSON processing failed.
    #[error("registration: invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// Add-on compression failed.
    #[error("registration: add-on encoding failed: {0}")]
    Io(#[from] std::io::Error),
    /// The registration service returned malformed data.
    #[error("registration: invalid response: {0}")]
    InvalidResponse(String),
    /// The user rejected app creation or authorization.
    #[error("registration denied ({code}): {description}")]
    AccessDenied {
        /// Service error code.
        code: String,
        /// Service error description.
        description: String,
    },
    /// The device-code session expired.
    #[error("registration expired ({code}): {description}")]
    Expired {
        /// Service error code.
        code: String,
        /// Service error description.
        description: String,
    },
    /// Another service-defined registration error occurred.
    #[error("registration failed ({code}): {description}")]
    Service {
        /// Service error code.
        code: String,
        /// Service error description.
        description: String,
    },
}

/// App metadata pre-filled into the application creation page.
#[derive(Clone, Debug, Default)]
pub struct AppPreset {
    /// One to six avatar URL candidates. The first candidate is selected by default.
    pub avatars: Option<Vec<String>>,
    /// Application name. The `{user}` placeholder is supported by the web page.
    pub name: Option<String>,
    /// Application description. The `{user}` placeholder is supported by the web page.
    pub description: Option<String>,
}

impl AppPreset {
    /// Creates an empty preset.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets one avatar candidate.
    pub fn avatar(mut self, avatar: impl Into<String>) -> Self {
        self.avatars = Some(vec![avatar.into()]);
        self
    }

    /// Sets multiple avatar candidates.
    pub fn avatars<I, S>(mut self, avatars: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.avatars = Some(avatars.into_iter().map(Into::into).collect());
        self
    }

    /// Sets the application name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the application description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Incremental permissions, events and callbacks shown on the confirmation page.
#[derive(Clone, Debug, Default)]
pub struct AppAddons {
    /// `Some(false)` selects the minimal base template; `None` uses the platform default.
    pub preset: Option<bool>,
    /// Permission scopes.
    pub scopes: AppAddonsScopes,
    /// Event subscriptions.
    pub events: AppAddonsEvents,
    /// Callback subscriptions.
    pub callbacks: AppAddonsCallbacks,
}

/// Application-identity and user-identity permission scopes.
#[derive(Clone, Debug, Default)]
pub struct AppAddonsScopes {
    /// App-identity scopes, for example `im:message:send_as_bot`.
    pub tenant: Vec<String>,
    /// User-identity scopes.
    pub user: Vec<String>,
}

/// Event subscription lists.
#[derive(Clone, Debug, Default)]
pub struct AppAddonsEvents {
    /// App-identity events, for example `im.message.receive_v1`.
    pub tenant: Vec<String>,
    /// User-identity events.
    pub user: Vec<String>,
}

/// Callback subscription list.
#[derive(Clone, Debug, Default)]
pub struct AppAddonsCallbacks {
    /// Callback names, for example `card.action.trigger`.
    pub items: Vec<String>,
}

/// One-click registration options.
#[derive(Clone, Debug)]
pub struct RegistrationOptions {
    /// Source suffix appended to `rust-sdk` in the QR URL.
    pub source: Option<String>,
    /// Feishu account-domain override, mainly for tests or private deployment.
    pub domain: String,
    /// Lark account-domain override.
    pub lark_domain: String,
    /// Application metadata pre-fill.
    pub app_preset: Option<AppPreset>,
    /// Incremental permissions/events/callbacks.
    pub addons: Option<AppAddons>,
    /// Restricts the page to creating a new app.
    pub create_only: bool,
    /// Existing app ID used by platform-supported continuation flows.
    pub app_id: Option<String>,
}

impl Default for RegistrationOptions {
    fn default() -> Self {
        Self {
            source: None,
            domain: DEFAULT_FEISHU_DOMAIN.into(),
            lark_domain: DEFAULT_LARK_DOMAIN.into(),
            app_preset: None,
            addons: None,
            create_only: false,
            app_id: None,
        }
    }
}

impl RegistrationOptions {
    /// Creates default registration options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the source suffix.
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Sets metadata pre-fill.
    pub fn app_preset(mut self, app_preset: AppPreset) -> Self {
        self.app_preset = Some(app_preset);
        self
    }

    /// Sets incremental app configuration.
    pub fn addons(mut self, addons: AppAddons) -> Self {
        self.addons = Some(addons);
        self
    }

    /// Enables create-only mode.
    pub fn create_only(mut self, create_only: bool) -> Self {
        self.create_only = create_only;
        self
    }

    /// Sets an existing app ID.
    pub fn app_id(mut self, app_id: impl Into<String>) -> Self {
        self.app_id = Some(app_id.into());
        self
    }

    /// Overrides both account domains.
    pub fn domains(
        mut self,
        feishu_domain: impl Into<String>,
        lark_domain: impl Into<String>,
    ) -> Self {
        self.domain = feishu_domain.into();
        self.lark_domain = lark_domain.into();
        self
    }
}

/// QR code information emitted after the `begin` request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QrCodeInfo {
    /// URL that should be rendered as a QR code or opened in a browser.
    pub url: String,
    /// Number of seconds before the registration session expires.
    pub expire_in: u64,
}

/// Registration status emitted while polling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistrationStatus {
    /// The user has not completed the flow yet.
    Polling,
    /// The service requested a slower polling rate.
    SlowDown {
        /// New polling interval in seconds.
        interval: u64,
    },
    /// The scanning tenant is a Lark tenant and polling switched domains.
    DomainSwitched,
}

/// User information returned by the registration service.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct RegisteredUserInfo {
    /// User open ID.
    pub open_id: Option<String>,
    /// `feishu` or `lark` tenant brand.
    pub tenant_brand: Option<String>,
}

/// Credentials returned after successful one-click app creation.
#[derive(Clone, Deserialize, Eq, PartialEq)]
pub struct RegisterAppResult {
    /// Created application ID.
    pub client_id: String,
    /// Created application secret.
    pub client_secret: String,
    /// Scanning user information, when requested and returned.
    pub user_info: Option<RegisteredUserInfo>,
}

impl fmt::Debug for RegisterAppResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RegisterAppResult")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .field("user_info", &self.user_info)
            .finish()
    }
}

/// Active device-code registration session.
pub struct RegistrationSession {
    http: reqwest::Client,
    device_code: String,
    current_domain: String,
    lark_domain: String,
    interval: u64,
    expires_at: Instant,
    domain_switched: bool,
    qr_code: QrCodeInfo,
}

impl RegistrationSession {
    /// Returns QR code data for this session.
    pub fn qr_code(&self) -> &QrCodeInfo {
        &self.qr_code
    }

    /// Polls until credentials are returned or the flow fails.
    pub async fn wait(self) -> Result<RegisterAppResult, RegistrationError> {
        self.wait_with_status(|_| {}).await
    }

    /// Polls and reports status changes.
    pub async fn wait_with_status<F>(
        mut self,
        mut on_status_change: F,
    ) -> Result<RegisterAppResult, RegistrationError>
    where
        F: FnMut(&RegistrationStatus),
    {
        let mut wait_before_poll = false;

        loop {
            self.ensure_not_expired()?;
            if wait_before_poll {
                self.sleep_until_next_poll().await?;
            }
            wait_before_poll = true;

            let response: PollResponse = post_form(
                &self.http,
                &self.current_domain,
                &[
                    ("action", "poll".to_owned()),
                    ("device_code", self.device_code.clone()),
                ],
            )
            .await?;

            if !self.domain_switched
                && response
                    .user_info
                    .as_ref()
                    .and_then(|info| info.tenant_brand.as_deref())
                    == Some("lark")
            {
                self.current_domain = self.lark_domain.clone();
                self.domain_switched = true;
                on_status_change(&RegistrationStatus::DomainSwitched);
                wait_before_poll = false;
                continue;
            }

            if let (Some(client_id), Some(client_secret)) =
                (response.client_id, response.client_secret)
            {
                if !client_id.is_empty() && !client_secret.is_empty() {
                    return Ok(RegisterAppResult {
                        client_id,
                        client_secret,
                        user_info: response.user_info,
                    });
                }
            }

            match response.error.as_deref().unwrap_or_default() {
                "authorization_pending" => {
                    on_status_change(&RegistrationStatus::Polling);
                }
                "slow_down" => {
                    self.interval += 5;
                    on_status_change(&RegistrationStatus::SlowDown {
                        interval: self.interval,
                    });
                }
                "access_denied" => {
                    return Err(RegistrationError::AccessDenied {
                        code: "access_denied".into(),
                        description: response
                            .error_description
                            .unwrap_or_else(|| "user denied registration".into()),
                    });
                }
                "expired_token" => {
                    return Err(RegistrationError::Expired {
                        code: "expired_token".into(),
                        description: response
                            .error_description
                            .unwrap_or_else(|| "registration expired".into()),
                    });
                }
                "" => {
                    // Keep polling for compatibility with the Go implementation.
                }
                code => {
                    return Err(RegistrationError::Service {
                        code: code.to_owned(),
                        description: response
                            .error_description
                            .unwrap_or_else(|| "unknown registration error".into()),
                    });
                }
            }
        }
    }

    fn ensure_not_expired(&self) -> Result<(), RegistrationError> {
        if Instant::now() >= self.expires_at {
            return Err(expired_error());
        }
        Ok(())
    }

    async fn sleep_until_next_poll(&self) -> Result<(), RegistrationError> {
        let remaining = self.expires_at.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(expired_error());
        }
        if Duration::from_secs(self.interval) >= remaining {
            sleep(remaining).await;
            return Err(expired_error());
        }
        sleep(Duration::from_secs(self.interval)).await;
        Ok(())
    }
}

/// Starts one-click registration and returns a session containing the QR URL.
pub async fn begin_registration(
    options: RegistrationOptions,
) -> Result<RegistrationSession, RegistrationError> {
    validate_options(&options)?;
    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(20))
        .build()?;

    let response: BeginResponse = post_form(
        &http,
        &options.domain,
        &[
            ("action", "begin".to_owned()),
            ("archetype", "PersonalAgent".to_owned()),
            ("auth_method", "client_secret".to_owned()),
            ("request_user_info", "open_id".to_owned()),
        ],
    )
    .await?;

    if response.device_code.trim().is_empty() {
        return Err(RegistrationError::InvalidResponse(
            "device_code is empty".into(),
        ));
    }
    if response.verification_uri_complete.trim().is_empty() {
        return Err(RegistrationError::InvalidResponse(
            "verification_uri_complete is empty".into(),
        ));
    }

    let interval = if response.interval == 0 {
        DEFAULT_POLL_INTERVAL_SECONDS
    } else {
        response.interval
    };
    let expire_in = if response.expire_in == 0 {
        DEFAULT_EXPIRE_SECONDS
    } else {
        response.expire_in
    };
    let qr_url = build_qr_code_url(&response.verification_uri_complete, &options)?;

    Ok(RegistrationSession {
        http,
        device_code: response.device_code,
        current_domain: trim_domain(&options.domain),
        lark_domain: trim_domain(&options.lark_domain),
        interval,
        expires_at: Instant::now() + Duration::from_secs(expire_in),
        domain_switched: false,
        qr_code: QrCodeInfo {
            url: qr_url,
            expire_in,
        },
    })
}

/// Convenience API matching the callback shape of the official Java/Go SDKs.
pub async fn register_app<Q, S>(
    options: RegistrationOptions,
    on_qr_code: Q,
    on_status_change: S,
) -> Result<RegisterAppResult, RegistrationError>
where
    Q: FnOnce(&QrCodeInfo),
    S: FnMut(&RegistrationStatus),
{
    let session = begin_registration(options).await?;
    on_qr_code(session.qr_code());
    session.wait_with_status(on_status_change).await
}

/// Validates and encodes add-ons as gzip-compressed, URL-safe base64 without padding.
pub fn encode_addons(addons: &AppAddons) -> Result<String, RegistrationError> {
    let normalized = normalize_addons(addons)?;
    let json = serde_json::to_vec(&normalized)?;
    let mut encoder = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::default());
    encoder.write_all(&json)?;
    let compressed = encoder.finish()?;
    Ok(URL_SAFE_NO_PAD.encode(compressed))
}

fn validate_options(options: &RegistrationOptions) -> Result<(), RegistrationError> {
    validate_domain(&options.domain, "domain")?;
    validate_domain(&options.lark_domain, "lark_domain")?;
    if options
        .app_id
        .as_ref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(RegistrationError::InvalidArgument(
            "app_id must be a non-empty string".into(),
        ));
    }
    if let Some(preset) = options.app_preset.as_ref() {
        validate_app_preset(preset)?;
    }
    if let Some(addons) = options.addons.as_ref() {
        normalize_addons(addons)?;
    }
    Ok(())
}

fn validate_domain(domain: &str, field: &str) -> Result<(), RegistrationError> {
    let parsed = Url::parse(domain)?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(RegistrationError::InvalidArgument(format!(
            "{field} must be an absolute HTTP(S) URL"
        )));
    }
    Ok(())
}

fn validate_app_preset(preset: &AppPreset) -> Result<(), RegistrationError> {
    if let Some(avatars) = preset.avatars.as_ref() {
        if avatars.is_empty() {
            return Err(RegistrationError::InvalidArgument(
                "app_preset.avatars must contain at least one URL".into(),
            ));
        }
        if avatars.len() > AVATAR_MAX_COUNT {
            return Err(RegistrationError::InvalidArgument(format!(
                "app_preset.avatars supports at most {AVATAR_MAX_COUNT} URLs, got {}",
                avatars.len()
            )));
        }
        for (index, avatar) in avatars.iter().enumerate() {
            if avatar.trim().is_empty() {
                return Err(RegistrationError::InvalidArgument(format!(
                    "app_preset.avatars[{index}] must be a non-empty string"
                )));
            }
        }
    }
    Ok(())
}

fn build_qr_code_url(
    raw_url: &str,
    options: &RegistrationOptions,
) -> Result<String, RegistrationError> {
    let mut url = Url::parse(raw_url)?;
    let managed_keys = [
        "from",
        "tp",
        "source",
        "avatar",
        "name",
        "desc",
        "addons",
        "createOnly",
        "clientID",
    ];
    let mut pairs: Vec<(String, String)> = url
        .query_pairs()
        .into_owned()
        .filter(|(key, _)| !managed_keys.contains(&key.as_str()))
        .collect();

    pairs.push(("from".into(), "sdk".into()));
    pairs.push(("tp".into(), "sdk".into()));
    let source = options
        .source
        .as_deref()
        .filter(|source| !source.trim().is_empty())
        .map(|source| format!("{SDK_NAME}/{source}"))
        .unwrap_or_else(|| SDK_NAME.into());
    pairs.push(("source".into(), source));

    if let Some(preset) = options.app_preset.as_ref() {
        if let Some(avatars) = preset.avatars.as_ref() {
            for avatar in avatars {
                pairs.push(("avatar".into(), avatar.clone()));
            }
        }
        if let Some(name) = preset.name.as_ref() {
            pairs.push(("name".into(), name.clone()));
        }
        if let Some(description) = preset.description.as_ref() {
            pairs.push(("desc".into(), description.clone()));
        }
    }
    if let Some(addons) = options.addons.as_ref() {
        pairs.push(("addons".into(), encode_addons(addons)?));
    }
    if options.create_only {
        pairs.push(("createOnly".into(), "true".into()));
    }
    if let Some(app_id) = options.app_id.as_ref() {
        pairs.push(("clientID".into(), app_id.clone()));
    }

    url.set_query(None);
    url.query_pairs_mut().extend_pairs(pairs);
    Ok(url.to_string())
}

async fn post_form<T: for<'de> Deserialize<'de>>(
    http: &reqwest::Client,
    domain: &str,
    fields: &[(&str, String)],
) -> Result<T, RegistrationError> {
    let endpoint = format!("{}{}", trim_domain(domain), ENDPOINT);
    let response = http.post(endpoint).form(fields).send().await?;
    let status = response.status();
    let body = response.text().await?;
    if body.trim().is_empty() {
        return Err(RegistrationError::InvalidResponse(
            "empty response body".into(),
        ));
    }
    if !status.is_success() {
        return Err(RegistrationError::InvalidResponse(format!(
            "HTTP {status}: {}",
            truncate(&body)
        )));
    }
    serde_json::from_str(&body).map_err(Into::into)
}

fn trim_domain(domain: &str) -> String {
    domain.trim_end_matches('/').to_owned()
}

fn expired_error() -> RegistrationError {
    RegistrationError::Expired {
        code: "expired_token".into(),
        description: "registration expired".into(),
    }
}

fn truncate(value: &str) -> String {
    const MAX_CHARS: usize = 2_048;
    if value.chars().count() <= MAX_CHARS {
        value.to_owned()
    } else {
        format!("{}…", value.chars().take(MAX_CHARS).collect::<String>())
    }
}

#[derive(Debug, Deserialize)]
struct BeginResponse {
    device_code: String,
    verification_uri_complete: String,
    #[serde(default)]
    interval: u64,
    #[serde(default, alias = "expires_in")]
    expire_in: u64,
}

#[derive(Debug, Deserialize)]
struct PollResponse {
    client_id: Option<String>,
    client_secret: Option<String>,
    user_info: Option<RegisteredUserInfo>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Serialize)]
struct NormalizedAddons<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    preset: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scopes: Option<NormalizedScopes<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    events: Option<NormalizedEvents<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    callbacks: Option<NormalizedCallbacks<'a>>,
}

#[derive(Serialize)]
struct NormalizedScopes<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    tenant: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<&'a [String]>,
}

#[derive(Serialize)]
struct NormalizedEvents<'a> {
    items: NormalizedEventItems<'a>,
}

#[derive(Serialize)]
struct NormalizedEventItems<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    tenant: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<&'a [String]>,
}

#[derive(Serialize)]
struct NormalizedCallbacks<'a> {
    items: &'a [String],
}

fn normalize_addons(addons: &AppAddons) -> Result<NormalizedAddons<'_>, RegistrationError> {
    validate_strings(&addons.scopes.tenant, "addons.scopes.tenant")?;
    validate_strings(&addons.scopes.user, "addons.scopes.user")?;
    validate_strings(&addons.events.tenant, "addons.events.items.tenant")?;
    validate_strings(&addons.events.user, "addons.events.items.user")?;
    validate_strings(&addons.callbacks.items, "addons.callbacks.items")?;

    let item_count = addons.scopes.tenant.len()
        + addons.scopes.user.len()
        + addons.events.tenant.len()
        + addons.events.user.len()
        + addons.callbacks.items.len();
    if item_count == 0 && addons.preset != Some(false) {
        return Err(RegistrationError::InvalidArgument(
            "addons must contain at least one scope, event or callback, unless preset is false"
                .into(),
        ));
    }

    let scopes = if addons.scopes.tenant.is_empty() && addons.scopes.user.is_empty() {
        None
    } else {
        Some(NormalizedScopes {
            tenant: (!addons.scopes.tenant.is_empty()).then_some(addons.scopes.tenant.as_slice()),
            user: (!addons.scopes.user.is_empty()).then_some(addons.scopes.user.as_slice()),
        })
    };
    let events = if addons.events.tenant.is_empty() && addons.events.user.is_empty() {
        None
    } else {
        Some(NormalizedEvents {
            items: NormalizedEventItems {
                tenant: (!addons.events.tenant.is_empty())
                    .then_some(addons.events.tenant.as_slice()),
                user: (!addons.events.user.is_empty()).then_some(addons.events.user.as_slice()),
            },
        })
    };
    let callbacks = (!addons.callbacks.items.is_empty()).then_some(NormalizedCallbacks {
        items: addons.callbacks.items.as_slice(),
    });

    Ok(NormalizedAddons {
        preset: addons.preset,
        scopes,
        events,
        callbacks,
    })
}

fn validate_strings(values: &[String], path: &str) -> Result<(), RegistrationError> {
    for (index, value) in values.iter().enumerate() {
        if value.is_empty() {
            return Err(RegistrationError::InvalidArgument(format!(
                "{path}[{index}] must be a non-empty string"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use flate2::read::GzDecoder;
    use pretty_assertions::assert_eq;
    use std::io::Read;

    #[test]
    fn qr_url_contains_presets_and_sdk_source() {
        let options = RegistrationOptions::new()
            .source("create-an-app-in-one-click-rust")
            .app_preset(
                AppPreset::new()
                    .avatars(["https://example.com/a.png", "https://example.com/b.png"])
                    .name("{user}的智能体")
                    .description("由 Rust SDK 创建"),
            )
            .create_only(true);

        let value = build_qr_code_url("https://example.com/scan?existing=1", &options).unwrap();
        let parsed = Url::parse(&value).unwrap();
        let query: Vec<(String, String)> = parsed.query_pairs().into_owned().collect();

        assert!(query.contains(&("existing".into(), "1".into())));
        assert!(query.contains(&(
            "source".into(),
            "rust-sdk/create-an-app-in-one-click-rust".into()
        )));
        assert_eq!(
            query
                .iter()
                .filter(|(key, _)| key == "avatar")
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>(),
            vec!["https://example.com/a.png", "https://example.com/b.png"]
        );
        assert!(query.contains(&("createOnly".into(), "true".into())));
    }

    #[test]
    fn addons_are_gzip_url_safe_base64() {
        let addons = AppAddons {
            scopes: AppAddonsScopes {
                tenant: vec!["im:message:send_as_bot".into()],
                user: Vec::new(),
            },
            events: AppAddonsEvents {
                tenant: vec!["im.message.receive_v1".into()],
                user: Vec::new(),
            },
            callbacks: AppAddonsCallbacks {
                items: vec!["card.action.trigger".into()],
            },
            ..Default::default()
        };
        let encoded = encode_addons(&addons).unwrap();
        assert!(!encoded.contains('+') && !encoded.contains('/') && !encoded.contains('='));

        let compressed = URL_SAFE_NO_PAD.decode(encoded).unwrap();
        let mut decoder = GzDecoder::new(compressed.as_slice());
        let mut json = String::new();
        decoder.read_to_string(&mut json).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["scopes"]["tenant"][0], "im:message:send_as_bot");
        assert_eq!(
            value["events"]["items"]["tenant"][0],
            "im.message.receive_v1"
        );
        assert_eq!(value["callbacks"]["items"][0], "card.action.trigger");
    }

    #[test]
    fn empty_addons_require_minimal_preset() {
        assert!(encode_addons(&AppAddons::default()).is_err());
        assert!(encode_addons(&AppAddons {
            preset: Some(false),
            ..Default::default()
        })
        .is_ok());
    }
}
