use bytes::Bytes;
use http::{HeaderMap, StatusCode};

/// A decoded API response together with transport metadata.
#[derive(Debug)]
pub struct ApiResponse<T> {
    /// HTTP status code.
    pub status: StatusCode,
    /// Response headers.
    pub headers: HeaderMap,
    /// Feishu/Lark request or log ID, when present.
    pub request_id: Option<String>,
    /// Decoded response body.
    pub body: T,
}

impl<T> ApiResponse<T> {
    pub(crate) fn map<U>(self, body: U) -> ApiResponse<U> {
        ApiResponse {
            status: self.status,
            headers: self.headers,
            request_id: self.request_id,
            body,
        }
    }
}

impl ApiResponse<Bytes> {
    pub(crate) fn new_bytes(
        status: StatusCode,
        headers: HeaderMap,
        request_id: Option<String>,
        body: Bytes,
    ) -> Self {
        Self {
            status,
            headers,
            request_id,
            body,
        }
    }
}
