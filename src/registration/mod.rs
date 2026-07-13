//! One-click Feishu/Lark application registration.
//!
//! Implements the device-code flow used by the official Java, Go and Python
//! SDKs, including QR presets, add-on encoding and automatic Feishu/Lark
//! account-domain switching.

use std::{fmt, io::Write, time::Duration};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use flate2::{Compression, GzBuilder};
use serde::Deserialize;
use serde_json::{Map, Value};
use thiserror::Error;
use tokio::time::{sleep, Instant};
use url::Url;

const SDK_NAME: &str = "rust-sdk";
const FEISHU_ACCOUNTS: &str = "https://accounts.feishu.cn";
const LARK_ACCOUNTS: &str = "https://accounts.larksuite.com";
const ENDPOINT: &str = "/oauth/v1/app/registration";
const DEFAULT_INTERVAL: u64 = 5;
const DEFAULT_EXPIRE: u64 = 600;
const MAX_AVATARS: usize = 6;

/// Errors returned by the registration flow.
#[derive(Debug, Error)]
pub enum RegistrationError {
    #[error("registration: invalid argument: {0}")]
    InvalidArgument(String),
    #[error("registration: network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("registration: invalid URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("registration: invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("registration: add-on encoding failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("registration: invalid response: {0}")]
    InvalidResponse(String),
    #[error("registration denied ({code}): {description}")]
    AccessDenied { code: String, description: String },
    #[error("registration expired ({code}): {description}")]
    Expired { code: String, description: String },
    #[error("registration failed ({code}): {description}")]
    Service { code: String, description: String },
}

/// App metadata pre-filled into the creation page.
#[derive(Clone, Debug, Default)]
pub struct AppPreset {
    pub avatars: Option<Vec<String>>,
    pub name: Option<String>,
    pub description: Option<String>,
}

impl AppPreset {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn avatar(mut self, value: impl Into<String>) -> Self {
        self.avatars = Some(vec![value.into()]);
        self
    }
    pub fn avatars<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.avatars = Some(values.into_iter().map(Into::into).collect());
        self
    }
    pub fn name(mut self, value: impl Into<String>) -> Self {
        self.name = Some(value.into());
        self
    }
    pub fn description(mut self, value: impl Into<String>) -> Self {
        self.description = Some(value.into());
        self
    }
}

/// Incremental app configuration encoded into the confirmation URL.
#[derive(Clone, Debug, Default)]
pub struct AppAddons {
    pub preset: Option<bool>,
    pub scopes: AppAddonsScopes,
    pub events: AppAddonsEvents,
    pub callbacks: AppAddonsCallbacks,
}

#[derive(Clone, Debug, Default)]
pub struct AppAddonsScopes {
    pub tenant: Vec<String>,
    pub user: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct AppAddonsEvents {
    pub tenant: Vec<String>,
    pub user: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct AppAddonsCallbacks {
    pub items: Vec<String>,
}

/// Registration options.
#[derive(Clone, Debug)]
pub struct RegistrationOptions {
    pub source: Option<String>,
    pub domain: String,
    pub lark_domain: String,
    pub app_preset: Option<AppPreset>,
    pub addons: Option<AppAddons>,
    pub create_only: bool,
    pub app_id: Option<String>,
}

impl Default for RegistrationOptions {
    fn default() -> Self {
        Self {
            source: None,
            domain: FEISHU_ACCOUNTS.into(),
            lark_domain: LARK_ACCOUNTS.into(),
            app_preset: None,
            addons: None,
            create_only: false,
            app_id: None,
        }
    }
}

impl RegistrationOptions {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn source(mut self, value: impl Into<String>) -> Self {
        self.source = Some(value.into());
        self
    }
    pub fn app_preset(mut self, value: AppPreset) -> Self {
        self.app_preset = Some(value);
        self
    }
    pub fn addons(mut self, value: AppAddons) -> Self {
        self.addons = Some(value);
        self
    }
    pub fn create_only(mut self, value: bool) -> Self {
        self.create_only = value;
        self
    }
    pub fn app_id(mut self, value: impl Into<String>) -> Self {
        self.app_id = Some(value.into());
        self
    }
    pub fn domains(mut self, feishu: impl Into<String>, lark: impl Into<String>) -> Self {
        self.domain = feishu.into();
        self.lark_domain = lark.into();
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QrCodeInfo {
    pub url: String,
    pub expire_in: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistrationStatus {
    Polling,
    SlowDown { interval: u64 },
    DomainSwitched,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct RegisteredUserInfo {
    pub open_id: Option<String>,
    pub tenant_brand: Option<String>,
}

#[derive(Clone, Deserialize, Eq, PartialEq)]
pub struct RegisterAppResult {
    pub client_id: String,
    pub client_secret: String,
    pub user_info: Option<RegisteredUserInfo>,
}

impl fmt::Debug for RegisterAppResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisterAppResult")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .field("user_info", &self.user_info)
            .finish()
    }
}

/// Starts and completes one-click registration.
pub async fn register_app<FQ, FS>(
    options: RegistrationOptions,
    on_qr_code: FQ,
    mut on_status_change: FS,
) -> Result<RegisterAppResult, RegistrationError>
where
    FQ: FnOnce(&QrCodeInfo),
    FS: FnMut(&RegistrationStatus),
{
    validate_options(&options)?;
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let begin: BeginResponse = post_form(
        &http,
        &options.domain,
        &[
            ("action", "begin"),
            ("archetype", "PersonalAgent"),
            ("auth_method", "client_secret"),
            ("request_user_info", "open_id"),
        ],
    )
    .await?;

    if begin.device_code.is_empty() || begin.verification_uri_complete.is_empty() {
        return Err(RegistrationError::InvalidResponse(
            "missing device_code or verification_uri_complete".into(),
        ));
    }

    let interval = begin.interval.max(DEFAULT_INTERVAL);
    let expire_in = begin.expire_in.max(DEFAULT_EXPIRE);
    let qr = QrCodeInfo {
        url: build_qr_url(&begin.verification_uri_complete, &options)?,
        expire_in,
    };
    on_qr_code(&qr);

    let mut domain = options.domain;
    let mut interval = interval;
    let deadline = Instant::now() + Duration::from_secs(expire_in);
    let mut switched = false;
    let mut wait_before_poll = false;

    loop {
        if Instant::now() >= deadline {
            return Err(expired("registration expired"));
        }
        if wait_before_poll {
            sleep(Duration::from_secs(interval)).await;
            if Instant::now() >= deadline {
                return Err(expired("polling timed out"));
            }
        }
        wait_before_poll = true;

        let poll: PollResponse = post_form(
            &http,
            &domain,
            &[("action", "poll"), ("device_code", &begin.device_code)],
        )
        .await?;

        if !switched
            && poll
                .user_info
                .as_ref()
                .and_then(|u| u.tenant_brand.as_deref())
                == Some("lark")
        {
            domain = options.lark_domain.clone();
            switched = true;
            wait_before_poll = false;
            on_status_change(&RegistrationStatus::DomainSwitched);
            continue;
        }

        if let (Some(client_id), Some(client_secret)) = (poll.client_id, poll.client_secret) {
            if !client_id.is_empty() && !client_secret.is_empty() {
                return Ok(RegisterAppResult {
                    client_id,
                    client_secret,
                    user_info: poll.user_info,
                });
            }
        }

        let code = poll.error.unwrap_or_default();
        let description = poll
            .error_description
            .unwrap_or_else(|| "unknown registration error".into());
        match code.as_str() {
            "authorization_pending" | "" => {
                on_status_change(&RegistrationStatus::Polling);
            }
            "slow_down" => {
                interval += 5;
                on_status_change(&RegistrationStatus::SlowDown { interval });
            }
            "access_denied" => {
                return Err(RegistrationError::AccessDenied { code, description });
            }
            "expired_token" => {
                return Err(RegistrationError::Expired { code, description });
            }
            _ => return Err(RegistrationError::Service { code, description }),
        }
    }
}

fn validate_options(options: &RegistrationOptions) -> Result<(), RegistrationError> {
    if options.domain.trim().is_empty() || options.lark_domain.trim().is_empty() {
        return Err(RegistrationError::InvalidArgument(
            "account domains must not be empty".into(),
        ));
    }
    if options.app_id.as_ref().is_some_and(|v| v.trim().is_empty()) {
        return Err(RegistrationError::InvalidArgument(
            "app_id must be a non-empty string".into(),
        ));
    }
    if let Some(avatars) = options
        .app_preset
        .as_ref()
        .and_then(|preset| preset.avatars.as_ref())
    {
        if avatars.is_empty() || avatars.len() > MAX_AVATARS {
            return Err(RegistrationError::InvalidArgument(format!(
                "app_preset.avatars must contain 1-{MAX_AVATARS} URLs"
            )));
        }
        validate_strings(avatars, "app_preset.avatars")?;
    }
    if let Some(addons) = options.addons.as_ref() {
        let _ = addons_json(addons)?;
    }
    Ok(())
}

fn build_qr_url(raw: &str, options: &RegistrationOptions) -> Result<String, RegistrationError> {
    let mut url = Url::parse(raw)?;
    let managed = [
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
        .filter(|(key, _)| !managed.contains(&key.as_str()))
        .collect();
    pairs.extend([
        ("from".into(), "sdk".into()),
        ("tp".into(), "sdk".into()),
        (
            "source".into(),
            options
                .source
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(|s| format!("{SDK_NAME}/{s}"))
                .unwrap_or_else(|| SDK_NAME.into()),
        ),
    ]);
    if let Some(preset) = options.app_preset.as_ref() {
        if let Some(avatars) = preset.avatars.as_ref() {
            pairs.extend(avatars.iter().cloned().map(|v| ("avatar".into(), v)));
        }
        if let Some(value) = preset.name.as_ref() {
            pairs.push(("name".into(), value.clone()));
        }
        if let Some(value) = preset.description.as_ref() {
            pairs.push(("desc".into(), value.clone()));
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

fn encode_addons(addons: &AppAddons) -> Result<String, RegistrationError> {
    let body = serde_json::to_vec(&addons_json(addons)?)?;
    let mut writer = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::default());
    writer.write_all(&body)?;
    Ok(URL_SAFE_NO_PAD.encode(writer.finish()?))
}

fn addons_json(addons: &AppAddons) -> Result<Value, RegistrationError> {
    validate_strings(&addons.scopes.tenant, "addons.scopes.tenant")?;
    validate_strings(&addons.scopes.user, "addons.scopes.user")?;
    validate_strings(&addons.events.tenant, "addons.events.items.tenant")?;
    validate_strings(&addons.events.user, "addons.events.items.user")?;
    validate_strings(&addons.callbacks.items, "addons.callbacks.items")?;
    let count = addons.scopes.tenant.len()
        + addons.scopes.user.len()
        + addons.events.tenant.len()
        + addons.events.user.len()
        + addons.callbacks.items.len();
    if count == 0 && addons.preset != Some(false) {
        return Err(RegistrationError::InvalidArgument(
            "addons must contain at least one scope, event or callback, unless preset is false"
                .into(),
        ));
    }

    let mut root = Map::new();
    if let Some(preset) = addons.preset {
        root.insert("preset".into(), Value::Bool(preset));
    }
    let scopes = identity_map(&addons.scopes.tenant, &addons.scopes.user);
    if !scopes.is_empty() {
        root.insert("scopes".into(), Value::Object(scopes));
    }
    let events = identity_map(&addons.events.tenant, &addons.events.user);
    if !events.is_empty() {
        let mut wrapper = Map::new();
        wrapper.insert("items".into(), Value::Object(events));
        root.insert("events".into(), Value::Object(wrapper));
    }
    if !addons.callbacks.items.is_empty() {
        let mut callbacks = Map::new();
        callbacks.insert(
            "items".into(),
            serde_json::to_value(&addons.callbacks.items)?,
        );
        root.insert("callbacks".into(), Value::Object(callbacks));
    }
    Ok(Value::Object(root))
}

fn identity_map(tenant: &[String], user: &[String]) -> Map<String, Value> {
    let mut map = Map::new();
    if !tenant.is_empty() {
        map.insert("tenant".into(), serde_json::to_value(tenant).unwrap());
    }
    if !user.is_empty() {
        map.insert("user".into(), serde_json::to_value(user).unwrap());
    }
    map
}

fn validate_strings(values: &[String], path: &str) -> Result<(), RegistrationError> {
    for (index, value) in values.iter().enumerate() {
        if value.trim().is_empty() {
            return Err(RegistrationError::InvalidArgument(format!(
                "{path}[{index}] must be a non-empty string"
            )));
        }
    }
    Ok(())
}

async fn post_form<T: for<'de> Deserialize<'de>>(
    http: &reqwest::Client,
    domain: &str,
    fields: &[(&str, &str)],
) -> Result<T, RegistrationError> {
    let endpoint = format!("{}{}", domain.trim_end_matches('/'), ENDPOINT);
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
    Ok(serde_json::from_str(&body)?)
}

fn expired(description: &str) -> RegistrationError {
    RegistrationError::Expired {
        code: "expired_token".into(),
        description: description.into(),
    }
}

fn truncate(value: &str) -> String {
    const MAX: usize = 2_048;
    if value.chars().count() <= MAX {
        value.into()
    } else {
        format!("{}…", value.chars().take(MAX).collect::<String>())
    }
}

#[derive(Deserialize)]
struct BeginResponse {
    device_code: String,
    verification_uri_complete: String,
    #[serde(default)]
    interval: u64,
    #[serde(default, alias = "expires_in")]
    expire_in: u64,
}

#[derive(Deserialize)]
struct PollResponse {
    client_id: Option<String>,
    client_secret: Option<String>,
    user_info: Option<RegisteredUserInfo>,
    error: Option<String>,
    error_description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::GzDecoder;
    use std::io::Read;

    #[test]
    fn qr_url_contains_presets_and_source() {
        let options = RegistrationOptions::new()
            .source("demo")
            .app_preset(AppPreset::new().avatars(["a", "b"]).name("{user} app"))
            .create_only(true);
        let value = build_qr_url("https://example.com/scan?existing=1", &options).unwrap();
        let query: Vec<_> = Url::parse(&value)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect();
        assert!(query.contains(&("source".into(), "rust-sdk/demo".into())));
        assert_eq!(query.iter().filter(|(k, _)| k == "avatar").count(), 2);
        assert!(query.contains(&("createOnly".into(), "true".into())));
    }

    #[test]
    fn addons_are_gzip_url_safe_base64() {
        let addons = AppAddons {
            scopes: AppAddonsScopes {
                tenant: vec!["im:message:send_as_bot".into()],
                user: vec![],
            },
            ..Default::default()
        };
        let encoded = encode_addons(&addons).unwrap();
        assert!(!encoded.contains(['+', '/', '=']));
        let bytes = URL_SAFE_NO_PAD.decode(encoded).unwrap();
        let mut decoder = GzDecoder::new(bytes.as_slice());
        let mut json = String::new();
        decoder.read_to_string(&mut json).unwrap();
        assert!(json.contains("im:message:send_as_bot"));
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
