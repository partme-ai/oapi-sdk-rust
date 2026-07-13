use serde::{Deserialize, Serialize};

use crate::{ApiRequest, ApiResponse, Client, Result};

/// Typed IM message API.
#[derive(Clone, Copy)]
pub struct MessageService<'a> {
    client: &'a Client,
}

impl<'a> MessageService<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Sends a message as the application bot.
    ///
    /// `receive_id_type` is typically `open_id`, `user_id`, `union_id`,
    /// `email` or `chat_id`.
    pub async fn create(
        &self,
        receive_id_type: impl Into<String>,
        request: &CreateMessageRequest,
    ) -> Result<ApiResponse<CreateMessageResponse>> {
        let api_request = ApiRequest::post("/open-apis/im/v1/messages")
            .query("receive_id_type", receive_id_type.into())
            .tenant_access_token()
            .json(request)?;
        self.client.execute(api_request).await
    }
}

/// Request body for sending a message.
#[derive(Clone, Debug, Serialize)]
pub struct CreateMessageRequest {
    /// Receiver identifier selected by `receive_id_type`.
    pub receive_id: String,
    /// Message type, for example `text`, `post`, `image` or `interactive`.
    pub msg_type: String,
    /// JSON-encoded message content required by the Feishu/Lark API.
    pub content: String,
    /// Optional idempotency UUID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
}

impl CreateMessageRequest {
    /// Creates a text message request and serializes the text content correctly.
    pub fn text(receive_id: impl Into<String>, text: impl Into<String>) -> Result<Self> {
        let content = serde_json::to_string(&serde_json::json!({ "text": text.into() }))?;
        Ok(Self {
            receive_id: receive_id.into(),
            msg_type: "text".into(),
            content,
            uuid: None,
        })
    }

    /// Adds an idempotency UUID.
    pub fn uuid(mut self, uuid: impl Into<String>) -> Self {
        self.uuid = Some(uuid.into());
        self
    }
}

/// Standard response from the create-message API.
#[derive(Clone, Debug, Deserialize)]
pub struct CreateMessageResponse {
    /// Platform result code.
    #[serde(default)]
    pub code: i64,
    /// Platform result message.
    #[serde(default)]
    pub msg: String,
    /// Created message data.
    pub data: Option<CreateMessageData>,
}

/// Subset of fields returned for a created message.
#[derive(Clone, Debug, Deserialize)]
pub struct CreateMessageData {
    pub message_id: Option<String>,
    pub root_id: Option<String>,
    pub parent_id: Option<String>,
    pub thread_id: Option<String>,
    pub msg_type: Option<String>,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
    pub chat_id: Option<String>,
}
