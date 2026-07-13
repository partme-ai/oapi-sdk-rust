//! Async Rust SDK for the Feishu/Lark Open Platform.
//!
//! The crate provides:
//! - a reusable OpenAPI transport with automatic access-token handling;
//! - typed service modules for commonly used APIs;
//! - one-click app registration compatible with the official Java/Go flow;
//! - event signature verification and encrypted callback decoding.

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod client;
pub mod core;
#[cfg(feature = "events")]
#[cfg_attr(docsrs, doc(cfg(feature = "events")))]
pub mod event;
#[cfg(feature = "registration")]
#[cfg_attr(docsrs, doc(cfg(feature = "registration")))]
pub mod registration;
pub mod service;

pub use client::{Client, ClientBuilder};
pub use core::{
    AccessTokenType, ApiRequest, ApiResponse, AppType, Error, MemoryTokenCache, MultipartField,
    RequestBody, Result, TokenCache,
};
