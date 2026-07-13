use http::StatusCode;
use thiserror::Error;

/// SDK result type.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors returned by the OpenAPI transport.
#[derive(Debug, Error)]
pub enum Error {
    /// A client-side argument is invalid.
    #[error("invalid parameter: {0}")]
    InvalidParameter(String),

    /// A URL could not be parsed or constructed.
    #[error("invalid URL: {0}")]
    Url(#[from] url::ParseError),

    /// HTTP transport failed.
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization or deserialization failed.
    #[error("JSON processing failed: {0}")]
    Json(#[from] serde_json::Error),

    /// The HTTP status was not successful and no structured platform error was available.
    #[error("HTTP status {status}: {body}")]
    HttpStatus {
        /// HTTP status code.
        status: StatusCode,
        /// Truncated response body.
        body: String,
    },

    /// The platform returned a non-zero OpenAPI error code.
    #[error("OpenAPI error {code}: {message}")]
    Api {
        /// Platform error code.
        code: i64,
        /// Platform error message.
        message: String,
        /// Request/log identifier supplied by the platform.
        request_id: Option<String>,
    },

    /// A required access token was not supplied and cannot be generated.
    #[error("missing {0} access token")]
    MissingAccessToken(&'static str),

    /// A custom token cache failed.
    #[error("token cache error: {0}")]
    TokenCache(String),

    /// The response body was empty or malformed.
    #[error("invalid response: {0}")]
    InvalidResponse(String),
}
