use std::collections::BTreeMap;

use bytes::Bytes;
use http::{HeaderMap, HeaderName, HeaderValue, Method};
use serde::Serialize;
use serde_json::Value;

use super::{Error, Result};

/// Access token identity required by an API.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AccessTokenType {
    /// The API does not require an access token.
    #[default]
    None,
    /// Application access token.
    App,
    /// Tenant access token.
    Tenant,
    /// User access token supplied by the caller.
    User,
}

/// One multipart/form-data field.
#[derive(Clone, Debug)]
pub enum MultipartField {
    /// Plain text field.
    Text {
        /// Form field name.
        name: String,
        /// Form field value.
        value: String,
    },
    /// In-memory file field.
    File {
        /// Form field name.
        name: String,
        /// File name sent to the server.
        file_name: String,
        /// Optional MIME type.
        mime_type: Option<String>,
        /// File bytes.
        data: Bytes,
    },
}

/// Request payload.
#[derive(Clone, Debug, Default)]
pub enum RequestBody {
    /// No body.
    #[default]
    Empty,
    /// JSON body.
    Json(Value),
    /// application/x-www-form-urlencoded body.
    Form(Vec<(String, String)>),
    /// Raw bytes and optional content type.
    Bytes {
        /// Raw request body.
        data: Bytes,
        /// Content-Type header value.
        content_type: Option<String>,
    },
    /// multipart/form-data body.
    Multipart(Vec<MultipartField>),
}

/// Generic OpenAPI request builder.
#[derive(Clone, Debug)]
pub struct ApiRequest {
    pub(crate) method: Method,
    pub(crate) path: String,
    pub(crate) path_params: BTreeMap<String, String>,
    pub(crate) query: Vec<(String, String)>,
    pub(crate) headers: HeaderMap,
    pub(crate) body: RequestBody,
    pub(crate) access_token_type: AccessTokenType,
    pub(crate) access_token: Option<String>,
    pub(crate) tenant_key: Option<String>,
    pub(crate) app_ticket: Option<String>,
    pub(crate) request_id: Option<String>,
}

impl ApiRequest {
    /// Creates a request with no authentication by default.
    pub fn new(method: Method, path: impl Into<String>) -> Self {
        Self {
            method,
            path: path.into(),
            path_params: BTreeMap::new(),
            query: Vec::new(),
            headers: HeaderMap::new(),
            body: RequestBody::Empty,
            access_token_type: AccessTokenType::None,
            access_token: None,
            tenant_key: None,
            app_ticket: None,
            request_id: None,
        }
    }

    /// Creates a GET request.
    pub fn get(path: impl Into<String>) -> Self {
        Self::new(Method::GET, path)
    }

    /// Creates a POST request.
    pub fn post(path: impl Into<String>) -> Self {
        Self::new(Method::POST, path)
    }

    /// Creates a PUT request.
    pub fn put(path: impl Into<String>) -> Self {
        Self::new(Method::PUT, path)
    }

    /// Creates a PATCH request.
    pub fn patch(path: impl Into<String>) -> Self {
        Self::new(Method::PATCH, path)
    }

    /// Creates a DELETE request.
    pub fn delete(path: impl Into<String>) -> Self {
        Self::new(Method::DELETE, path)
    }

    /// Adds a path variable. Both `:name` and `{name}` segments are supported.
    pub fn path_param(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.path_params.insert(name.into(), value.into());
        self
    }

    /// Appends one query parameter. Call repeatedly for array values.
    pub fn query(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.query.push((name.into(), value.into()));
        self
    }

    /// Adds a request header.
    pub fn header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.headers.insert(name, value);
        self
    }

    /// Adds a request header parsed from strings.
    pub fn header_str(mut self, name: &str, value: &str) -> Result<Self> {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|error| Error::InvalidParameter(format!("invalid header name: {error}")))?;
        let value = HeaderValue::from_str(value)
            .map_err(|error| Error::InvalidParameter(format!("invalid header value: {error}")))?;
        self.headers.insert(name, value);
        Ok(self)
    }

    /// Serializes a JSON body.
    pub fn json<T: Serialize>(mut self, body: &T) -> Result<Self> {
        self.body = RequestBody::Json(serde_json::to_value(body)?);
        Ok(self)
    }

    /// Sets a form body.
    pub fn form<I, K, V>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.body = RequestBody::Form(
            fields
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        );
        self
    }

    /// Sets a raw byte body.
    pub fn bytes(mut self, data: impl Into<Bytes>, content_type: Option<String>) -> Self {
        self.body = RequestBody::Bytes {
            data: data.into(),
            content_type,
        };
        self
    }

    /// Sets a multipart body.
    pub fn multipart(mut self, fields: Vec<MultipartField>) -> Self {
        self.body = RequestBody::Multipart(fields);
        self
    }

    /// Requires an automatically managed app access token.
    pub fn app_access_token(mut self) -> Self {
        self.access_token_type = AccessTokenType::App;
        self
    }

    /// Requires an automatically managed tenant access token.
    pub fn tenant_access_token(mut self) -> Self {
        self.access_token_type = AccessTokenType::Tenant;
        self
    }

    /// Requires a caller-provided user access token.
    pub fn user_access_token(mut self, token: impl Into<String>) -> Self {
        self.access_token_type = AccessTokenType::User;
        self.access_token = Some(token.into());
        self
    }

    /// Uses an explicit app or tenant token instead of the token manager.
    pub fn explicit_access_token(
        mut self,
        token_type: AccessTokenType,
        token: impl Into<String>,
    ) -> Self {
        self.access_token_type = token_type;
        self.access_token = Some(token.into());
        self
    }

    /// Supplies a marketplace tenant key.
    pub fn tenant_key(mut self, tenant_key: impl Into<String>) -> Self {
        self.tenant_key = Some(tenant_key.into());
        self
    }

    /// Supplies an app ticket for marketplace token acquisition.
    pub fn app_ticket(mut self, app_ticket: impl Into<String>) -> Self {
        self.app_ticket = Some(app_ticket.into());
        self
    }

    /// Supplies an idempotency/request ID propagated as `Oapi-Sdk-Request-Id`.
    pub fn request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}
