//! IM v1 APIs.

pub mod message;

use crate::Client;

/// Entry point for IM v1 APIs.
#[derive(Clone, Copy)]
pub struct ImV1Service<'a> {
    client: &'a Client,
}

impl<'a> ImV1Service<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Message APIs.
    pub fn message(self) -> message::MessageService<'a> {
        message::MessageService::new(self.client)
    }
}
