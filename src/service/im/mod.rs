//! Instant Messaging APIs.

pub mod v1;

use crate::Client;

/// Entry point for IM APIs.
#[derive(Clone, Copy)]
pub struct ImService<'a> {
    client: &'a Client,
}

impl<'a> ImService<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// IM v1 API group.
    pub fn v1(self) -> v1::ImV1Service<'a> {
        v1::ImV1Service::new(self.client)
    }
}
