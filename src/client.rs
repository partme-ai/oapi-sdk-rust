use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use http::{
    header::{AUTHORIZATION, CONTENT_TYPE, USER_AGENT},
    HeaderMap, HeaderValue,
};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::de::DeserializeOwned;
use serde_json::Value;
use url::Url;

use crate::{
    core::{
        AccessTokenType, ApiRequest, ApiResponse, AppType, Config, Error, MemoryTokenCache,
        MultipartField, RequestBody, Result, TokenCache, TokenManager, FEISHU_BASE_URL,
    },
    service::im::ImService,
};

const INVALID_ACCESS_TOKEN_CODES: [i64; 3] = [99_991_671, 99_991_664, 99_991_663];

/// Builder for [`Client`].
pub struct ClientBuilder {
    app_id: String,
    app_secret: String,
    base_url: String,
    oauth_base_url: Option<String>,
    timeout: Duration,
    app_type: AppType,
    enable_token_cache: bool,
    token_cache: Option<Arc<dyn TokenCache>>,
    default_headers: HeaderMap,
    source: Option<String>,
    http_client: Option<reqwest::Client>,
}

impl ClientBuilder {
    pub(crate) fn new(app_id: impl Into<String>, app_secret: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            app_secret: app_secret.into(),
            base_url: FEISHU_BASE_URL.into(),
            oauth_base_url: None,
            timeout: Duration::from_secs(30),
            app_type: AppType::SelfBuilt,
            enable_token_cache: true,
            token_cache: None,
            default_headers: HeaderMap::new(),
            source: None,
            http_client: None,
        }
    }

    /// Overrides the OpenAPI base URL.
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Overrides the OAuth base URL.
    pub fn oauth_base_url(mut self, oauth_base_url: impl Into<String>) -> Self {
        self.oauth_base_url = Some(oauth_base_url.into());
        self
    }

    /// Configures the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Configures the application distribution type.
    pub fn app_type(mut self, app_type: AppType) -> Self {
        self.app_type = app_type;
        self
    }

    /// Enables or disables automatic token caching.
    pub fn enable_token_cache(mut self, enabled: bool) -> Self {
        self.enable_token_cache = enabled;
        self
    }

    /// Supplies a custom token cache.
    pub fn token_cache(mut self, cache: Arc<dyn TokenCache>) -> Self {
        self.token_cache = Some(cache);
        self
    }

    /// Adds a header sent with every API request.
    pub fn default_header(mut self, name: http::header::HeaderName, value: HeaderValue) -> Self {
        self.default_headers.insert(name, value);
        self
    }

    /// Adds an identifying source suffix to the User-Agent.
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Uses a caller-owned reqwest client.
    pub fn http_client(mut self, http_client: reqwest::Client) -> Self {
        self.http_client = Some(http_client);
        self
    }

    /// Validates configuration and creates the client.
    pub fn build(self) -> Result<Client> {
        if self.app_id.trim().is_empty() {
            return Err(Error::InvalidParameter("app_id must not be empty".into()));
        }
        let base_url = normalize_base_url(Url::parse(&self.base_url)?)?;
        let oauth_base_url = self
            .oauth_base_url
            .as_deref()
            .map(Url::parse)
            .transpose()?
            .map(normalize_base_url)
            .transpose()?;

        let config = Arc::new(Config {
            app_id: self.app_id,
            app_secret: self.app_secret,
            base_url,
            oauth_base_url,
            timeout: self.timeout,
            app_type: self.app_type,
            enable_token_cache: self.enable_token_cache,
            default_headers: self.default_headers,
            source: self.source,
        });

        let http = match self.http_client {
            Some(client) => client,
            None => reqwest::Client::builder().timeout(config.timeout).build()?,
        };
        let cache: Arc<dyn TokenCache> = self
            .token_cache
            .unwrap_or_else(|| Arc::new(MemoryTokenCache::default()));
        let token_manager = TokenManager::new(config.clone(), http.clone(), cache);

        Ok(Client {
            config,
            http,
            token_manager,
        })
    }
}

/// Feishu/Lark OpenAPI client.
#[derive(Clone)]
pub struct Client {
    config: Arc<Config>,
    http: reqwest::Client,
    token_manager: TokenManager,
}

impl Client {
    /// Starts building a client.
    pub fn builder(app_id: impl Into<String>, app_secret: impl Into<String>) -> ClientBuilder {
        ClientBuilder::new(app_id, app_secret)
    }

    /// Returns immutable client configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Returns the typed IM service.
    pub fn im(&self) -> ImService<'_> {
        ImService::new(self)
    }

    /// Executes an API request and deserializes the entire JSON response body.
    ///
    /// A top-level non-zero `code` is converted into [`Error::Api`].
    pub async fn execute<T: DeserializeOwned>(
        &self,
        request: ApiRequest,
    ) -> Result<ApiResponse<T>> {
        let raw = self.execute_json_value(request).await?;
        let decoded = serde_json::from_value(raw.body.clone())?;
        Ok(raw.map(decoded))
    }

    /// Executes an API request and returns the parsed JSON value.
    pub async fn execute_json_value(&self, request: ApiRequest) -> Result<ApiResponse<Value>> {
        for attempt in 0..2 {
            let managed_token = request.access_token.is_none()
                && matches!(
                    request.access_token_type,
                    AccessTokenType::App | AccessTokenType::Tenant
                );
            let token = self.resolve_token(&request).await?;
            let raw = self.send_once(&request, token.as_deref()).await?;
            let body_text = std::str::from_utf8(&raw.body).map_err(|error| {
                Error::InvalidResponse(format!("response is not UTF-8: {error}"))
            })?;
            if body_text.trim().is_empty() {
                return Err(Error::InvalidResponse("empty JSON response body".into()));
            }
            let value: Value = match serde_json::from_str(body_text) {
                Ok(value) => value,
                Err(_) if !raw.status.is_success() => {
                    return Err(Error::HttpStatus {
                        status: raw.status,
                        body: truncate(body_text),
                    });
                }
                Err(error) => return Err(error.into()),
            };
            let code = extract_code(&value).unwrap_or(0);

            if attempt == 0
                && managed_token
                && self.config.enable_token_cache
                && INVALID_ACCESS_TOKEN_CODES.contains(&code)
            {
                self.invalidate_token(&request).await?;
                continue;
            }

            if code != 0 {
                return Err(Error::Api {
                    code,
                    message: extract_message(&value),
                    request_id: raw.request_id,
                });
            }
            if !raw.status.is_success() {
                return Err(Error::HttpStatus {
                    status: raw.status,
                    body: truncate(body_text),
                });
            }
            return Ok(raw.map(value));
        }
        Err(Error::InvalidResponse(
            "request retry loop ended unexpectedly".into(),
        ))
    }

    /// Executes a request and returns raw response bytes.
    pub async fn execute_bytes(&self, request: ApiRequest) -> Result<ApiResponse<Bytes>> {
        let token = self.resolve_token(&request).await?;
        let response = self.send_once(&request, token.as_deref()).await?;
        if !response.status.is_success() {
            let body = String::from_utf8_lossy(&response.body);
            return Err(Error::HttpStatus {
                status: response.status,
                body: truncate(&body),
            });
        }
        Ok(response)
    }

    async fn resolve_token(&self, request: &ApiRequest) -> Result<Option<String>> {
        if let Some(token) = request.access_token.as_ref() {
            if token.trim().is_empty() {
                return Err(Error::InvalidParameter(
                    "explicit access token must not be empty".into(),
                ));
            }
            return Ok(Some(token.clone()));
        }
        match request.access_token_type {
            AccessTokenType::None => Ok(None),
            AccessTokenType::App => self
                .token_manager
                .app_access_token(request.app_ticket.as_deref())
                .await
                .map(Some),
            AccessTokenType::Tenant => self
                .token_manager
                .tenant_access_token(request.tenant_key.as_deref(), request.app_ticket.as_deref())
                .await
                .map(Some),
            AccessTokenType::User => Err(Error::MissingAccessToken("user")),
        }
    }

    async fn invalidate_token(&self, request: &ApiRequest) -> Result<()> {
        match request.access_token_type {
            AccessTokenType::App => self.token_manager.invalidate_app().await,
            AccessTokenType::Tenant => {
                self.token_manager
                    .invalidate_tenant(request.tenant_key.as_deref())
                    .await
            }
            AccessTokenType::None | AccessTokenType::User => Ok(()),
        }
    }

    async fn send_once(
        &self,
        request: &ApiRequest,
        access_token: Option<&str>,
    ) -> Result<ApiResponse<Bytes>> {
        let url = build_url(&self.config.base_url, request)?;
        let mut builder = self.http.request(request.method.clone(), url);

        let mut headers = self.config.default_headers.clone();
        for (name, value) in &request.headers {
            headers.insert(name.clone(), value.clone());
        }
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&self.config.user_agent()).map_err(|error| {
                Error::InvalidParameter(format!("invalid generated User-Agent: {error}"))
            })?,
        );
        if let Some(request_id) = request.request_id.as_deref() {
            headers.insert(
                http::header::HeaderName::from_static("oapi-sdk-request-id"),
                HeaderValue::from_str(request_id).map_err(|error| {
                    Error::InvalidParameter(format!("invalid request ID: {error}"))
                })?,
            );
        }
        if let Some(token) = access_token {
            let mut value = HeaderValue::from_str(&format!("Bearer {token}")).map_err(|error| {
                Error::InvalidParameter(format!("invalid access token header: {error}"))
            })?;
            value.set_sensitive(true);
            headers.insert(AUTHORIZATION, value);
        }
        builder = builder.headers(headers);

        builder = match &request.body {
            RequestBody::Empty => builder,
            RequestBody::Json(value) => builder.json(value),
            RequestBody::Form(fields) => builder.form(fields),
            RequestBody::Bytes { data, content_type } => {
                let mut next = builder.body(data.clone());
                if let Some(content_type) = content_type {
                    next = next.header(CONTENT_TYPE, content_type.as_str());
                }
                next
            }
            RequestBody::Multipart(fields) => {
                let mut form = reqwest::multipart::Form::new();
                for field in fields {
                    match field {
                        MultipartField::Text { name, value } => {
                            form = form.text(name.clone(), value.clone());
                        }
                        MultipartField::File {
                            name,
                            file_name,
                            mime_type,
                            data,
                        } => {
                            let mut part = reqwest::multipart::Part::bytes(data.clone().to_vec())
                                .file_name(file_name.clone());
                            if let Some(mime_type) = mime_type {
                                part = part.mime_str(mime_type).map_err(|error| {
                                    Error::InvalidParameter(format!(
                                        "invalid multipart MIME type: {error}"
                                    ))
                                })?;
                            }
                            form = form.part(name.clone(), part);
                        }
                    }
                }
                builder.multipart(form)
            }
        };

        tracing::debug!(
            method = %request.method,
            path = %request.path,
            "sending Feishu/Lark OpenAPI request"
        );
        let response = builder.send().await?;
        let status = response.status();
        let headers = response.headers().clone();
        let request_id = response_request_id(&headers);
        let body = response.bytes().await?;
        Ok(ApiResponse::new_bytes(status, headers, request_id, body))
    }
}

fn build_url(base_url: &Url, request: &ApiRequest) -> Result<Url> {
    let rendered_path = render_path(&request.path, &request.path_params)?;
    let mut url = if rendered_path.starts_with("http://") || rendered_path.starts_with("https://") {
        Url::parse(&rendered_path)?
    } else {
        base_url.join(rendered_path.trim_start_matches('/'))?
    };
    {
        let mut query = url.query_pairs_mut();
        for (name, value) in &request.query {
            query.append_pair(name, value);
        }
    }
    Ok(url)
}

fn render_path(path: &str, params: &std::collections::BTreeMap<String, String>) -> Result<String> {
    let mut output = Vec::new();
    for segment in path.split('/') {
        let variable = segment.strip_prefix(':').or_else(|| {
            segment
                .strip_prefix('{')
                .and_then(|value| value.strip_suffix('}'))
        });
        if let Some(variable) = variable {
            let value = params.get(variable).ok_or_else(|| {
                Error::InvalidParameter(format!("missing path parameter `{variable}`"))
            })?;
            if value.is_empty() {
                return Err(Error::InvalidParameter(format!(
                    "path parameter `{variable}` must not be empty"
                )));
            }
            output.push(utf8_percent_encode(value, NON_ALPHANUMERIC).to_string());
        } else {
            output.push(segment.to_owned());
        }
    }
    Ok(output.join("/"))
}

fn normalize_base_url(mut url: Url) -> Result<Url> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Error::InvalidParameter(
            "base URL must use http or https".into(),
        ));
    }
    if url.cannot_be_a_base() {
        return Err(Error::InvalidParameter(
            "base URL cannot be used as a URL base".into(),
        ));
    }
    if !url.path().ends_with('/') {
        let mut path = url.path().to_owned();
        path.push('/');
        url.set_path(&path);
    }
    Ok(url)
}

fn extract_code(value: &Value) -> Option<i64> {
    value.get("code").and_then(|code| {
        code.as_i64()
            .or_else(|| code.as_str().and_then(|value| value.parse().ok()))
    })
}

fn extract_message(value: &Value) -> String {
    ["msg", "message", "error_description", "error"]
        .into_iter()
        .find_map(|key| value.get(key).and_then(Value::as_str))
        .unwrap_or("OpenAPI request failed")
        .to_owned()
}

fn response_request_id(headers: &HeaderMap) -> Option<String> {
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
