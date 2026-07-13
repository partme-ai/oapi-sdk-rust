**English** | [简体中文](README.zh-CN.md)

# Feishu / Lark Open Platform Rust SDK

[![CI](https://github.com/partme-ai/oapi-sdk-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/partme-ai/oapi-sdk-rust/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue)](rust-toolchain.toml)
[![License](https://img.shields.io/badge/license-MIT-green)](LICENSE)

`oapi-sdk-rust` is an async Rust SDK for the Feishu/Lark Open Platform. It aligns its observable authentication, transport, event, and workflow behavior with the official Go, Python, and Java SDKs while exposing Rust-native builders, enums, traits, and async APIs.

The SDK removes repetitive work around access-token acquisition and caching, request construction, platform error decoding, file upload/download, callback signature verification, encrypted event decoding, and one-click app registration.

> Current version: `0.1.0`. The transport core, generic OpenAPI access, typed `im.v1.message.create`, event callback support, and one-click app registration are available. Full generated service coverage is still in progress; use `ApiRequest` for endpoints that do not yet have typed Rust wrappers.

## Documentation

- [One-click app creation guide](docs/create-an-app-in-one-click-rust.md)
- [Build and validation report](docs/VALIDATION.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Official SDK compatibility matrix](docs/COMPATIBILITY.md)
- [Roadmap](docs/ROADMAP.md)
- [Contributing](CONTRIBUTING.md)

## Feedback

Use [GitHub Issues](https://github.com/partme-ai/oapi-sdk-rust/issues) for defects, service coverage requests, and compatibility reports. Include the SDK/Rust versions, endpoint, token type, platform `code`/`msg`/`request_id`, and a minimal redacted reproduction.

Never post App Secrets, access tokens, App Tickets, Encrypt Keys, or full Authorization headers.

## Runtime requirements

- Rust `1.85` or later;
- Tokio async runtime;
- rustls TLS by default, without a system OpenSSL dependency;
- Linux, macOS, and Windows;
- Feishu connectivity to `open.feishu.cn` and `accounts.feishu.cn`;
- for Lark, set the API base URL to `open.larksuite.com`; app registration automatically switches to `accounts.larksuite.com` after detecting a Lark tenant.

## Installation

Before the first crates.io release:

```toml
[dependencies]
lark-oapi = { git = "https://github.com/partme-ai/oapi-sdk-rust", features = ["registration", "events"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```bash
cargo add lark-oapi --git https://github.com/partme-ai/oapi-sdk-rust --features registration,events
cargo add tokio --features macros,rt-multi-thread
```

Feature flags:

| Feature | Default | Purpose |
|---|---:|---|
| `rustls-tls` | yes | reqwest with rustls TLS |
| `native-tls` | no | reqwest with system Native TLS |
| `registration` | yes | one-click app registration and add-on encoding |
| `events` | yes | callback verification, AES decryption, and event parsing |

Minimal HTTP client:

```toml
lark-oapi = { git = "https://github.com/partme-ai/oapi-sdk-rust", default-features = false, features = ["rustls-tls"] }
```

## Quick start

### Build a self-built application client

```rust,no_run
use lark_oapi::Client;

# fn build() -> lark_oapi::Result<()> {
let client = Client::builder("cli_xxx", "app_secret")
    .source("my-service")
    .build()?;
# let _ = client;
# Ok(())
# }
```

App and tenant access tokens are acquired and cached automatically.

### Call a typed API

```rust,no_run
use lark_oapi::{service::im::v1::message::CreateMessageRequest, Client};

# async fn run() -> lark_oapi::Result<()> {
let client = Client::builder("cli_xxx", "app_secret").build()?;
let request = CreateMessageRequest::text("ou_xxx", "Hello from Rust")?
    .uuid("idempotency-uuid");

let response = client
    .im()
    .v1()
    .message()
    .create("open_id", &request)
    .await?;

println!("request_id={:?}", response.request_id);
println!("message={:?}", response.body.data);
# Ok(())
# }
```

### Call any OpenAPI endpoint

```rust,no_run
use lark_oapi::{ApiRequest, Client};
use serde_json::Value;

# async fn run() -> lark_oapi::Result<()> {
let client = Client::builder("cli_xxx", "app_secret").build()?;
let request = ApiRequest::get("/open-apis/contact/v3/users/:user_id")
    .path_param("user_id", "ou_xxx")
    .query("user_id_type", "open_id")
    .tenant_access_token()
    .request_id("business-operation-123");

let response = client.execute::<Value>(request).await?;
println!("status={}", response.status);
println!("request_id={:?}", response.request_id);
println!("body={}", response.body);
# Ok(())
# }
```

Both `:user_id` and `{user_id}` path variables are supported and percent-encoded.

### JSON, form, multipart, and bytes

```rust,no_run
use bytes::Bytes;
use lark_oapi::{ApiRequest, MultipartField};

# fn build() -> lark_oapi::Result<()> {
let json = ApiRequest::post("/open-apis/example/v1/resources")
    .tenant_access_token()
    .json(&serde_json::json!({ "name": "demo" }))?;

let form = ApiRequest::post("https://example.com/token")
    .form([("grant_type", "client_credentials")]);

let upload = ApiRequest::post("/open-apis/im/v1/files")
    .tenant_access_token()
    .multipart(vec![
        MultipartField::Text {
            name: "file_type".into(),
            value: "stream".into(),
        },
        MultipartField::File {
            name: "file".into(),
            file_name: "report.pdf".into(),
            mime_type: Some("application/pdf".into()),
            data: Bytes::from_static(b"file bytes"),
        },
    ]);
# let _ = (json, form, upload);
# Ok(())
# }
```

Use `Client::execute_bytes` for downloads and non-JSON success responses.

## Authentication

### Self-built applications

`.app_access_token()` and `.tenant_access_token()` use the SDK token manager. It checks the configured `TokenCache`, obtains a token on cache miss, reserves an expiry safety window, and invalidates/retries once for official invalid-token codes.

### User access tokens

User OAuth is intentionally outside the SDK transport. Supply the token explicitly:

```rust,no_run
use lark_oapi::ApiRequest;

let request = ApiRequest::get("/open-apis/calendar/v4/calendars")
    .user_access_token("u-xxxxxxxx");
```

### Explicit app or tenant tokens

```rust,no_run
use lark_oapi::{AccessTokenType, ApiRequest};

let request = ApiRequest::get("/open-apis/contact/v3/users")
    .explicit_access_token(AccessTokenType::Tenant, "t-xxxxxxxx");
```

When every request uses a caller-provided token, `app_secret` may be an empty string. It is required only when the SDK must obtain managed app/tenant tokens.

### Marketplace applications

```rust,no_run
use lark_oapi::{ApiRequest, AppType, Client};

# async fn run() -> lark_oapi::Result<()> {
let client = Client::builder("cli_xxx", "app_secret")
    .app_type(AppType::Marketplace)
    .build()?;

let request = ApiRequest::get("/open-apis/contact/v3/users")
    .tenant_access_token()
    .app_ticket("app_ticket_from_event_or_store")
    .tenant_key("tenant_key");

let _response = client.execute_json_value(request).await?;
# Ok(())
# }
```

Version 0.1 does not persist App Tickets. Store them securely in the application and supply them with marketplace requests.

## One-click app registration

The `registration` feature implements the application registration workflow used by the official Java, Go, and Python SDKs. It starts a device-code session, emits a verification URL, polls user confirmation, switches account domains for Lark tenants, and returns the created App ID/Secret.

### Run the complete Rust example

```bash
git clone https://github.com/partme-ai/oapi-sdk-rust.git
cd oapi-sdk-rust/examples/create-an-app-in-one-click-rust
cp .env.example .env
cargo run
```

The executable renders a terminal QR code and prints the credentials after explicit user confirmation.

See the complete guide, environment variables, protocol details, troubleshooting, and acceptance checklist:

**[docs/create-an-app-in-one-click-rust.md](docs/create-an-app-in-one-click-rust.md)**

### SDK example

```rust,no_run
use lark_oapi::registration::{
    register_app, AppAddons, AppAddonsCallbacks, AppAddonsEvents,
    AppAddonsScopes, AppPreset, RegistrationOptions, RegistrationStatus,
};

# async fn run() -> Result<(), Box<dyn std::error::Error>> {
let options = RegistrationOptions::new()
    .source("my-agent-cli")
    .app_preset(
        AppPreset::new()
            .avatars([
                "https://example.com/avatar-a.png",
                "https://example.com/avatar-b.png",
            ])
            .name("{user}'s agent")
            .description("Created by the Rust SDK"),
    )
    .addons(AppAddons {
        preset: None,
        scopes: AppAddonsScopes {
            tenant: vec!["im:message:send_as_bot".into()],
            user: vec!["calendar:calendar:read".into()],
        },
        events: AppAddonsEvents {
            tenant: vec!["im.message.receive_v1".into()],
            user: vec![],
        },
        callbacks: AppAddonsCallbacks {
            items: vec!["card.action.trigger".into()],
        },
    })
    .create_only(true);

let credentials = register_app(
    options,
    |qr| println!("Open or scan: {} (expires in {}s)", qr.url, qr.expire_in),
    |status| match status {
        RegistrationStatus::Polling => println!("Waiting for confirmation"),
        RegistrationStatus::SlowDown { interval } => {
            println!("Polling interval changed to {interval}s")
        }
        RegistrationStatus::DomainSwitched => println!("Switched to the Lark account domain"),
    },
)
.await?;

println!("app_id={}", credentials.client_id);
# Ok(())
# }
```

Registration add-ons are additive. `preset: None` or `Some(true)` keeps the platform default base template; `Some(false)` selects the minimal base template. Sensitive settings such as event URLs and encryption keys must be configured through the application configuration OpenAPI rather than QR parameters.

## Event callbacks

`EventParser` verifies callback signatures, decrypts AES-256-CBC payloads, verifies the configured token, and parses URL-verification challenges or business events.

```rust,no_run
use http::HeaderMap;
use lark_oapi::event::{EventParser, ParsedEvent};

# fn handle(headers: &HeaderMap, body: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
let parser = EventParser::new()
    .verification_token("verification-token")
    .encrypt_key("encrypt-key");

let response = match parser.parse(headers, body)? {
    ParsedEvent::Challenge { challenge } => {
        serde_json::json!({ "challenge": challenge }).to_string()
    }
    ParsedEvent::Event { event_type, event, .. } => {
        println!("event_type={event_type}, payload={event}");
        r#"{"code":0}"#.to_owned()
    }
};
# Ok(response)
# }
```

`skip_signature_verification(true)` is for controlled tests only and must not be enabled in production.

## Client configuration

```rust,no_run
use std::{sync::Arc, time::Duration};
use http::{HeaderName, HeaderValue};
use lark_oapi::{AppType, Client, MemoryTokenCache};

# fn build() -> lark_oapi::Result<()> {
let http_client = reqwest::Client::builder()
    .connect_timeout(Duration::from_secs(10))
    .build()?;

let client = Client::builder("cli_xxx", "app_secret")
    .base_url("https://open.larksuite.com")
    .oauth_base_url("https://open.larksuite.com")
    .timeout(Duration::from_secs(30))
    .app_type(AppType::SelfBuilt)
    .enable_token_cache(true)
    .token_cache(Arc::new(MemoryTokenCache::default()))
    .default_header(
        HeaderName::from_static("x-business-unit"),
        HeaderValue::from_static("agent-platform"),
    )
    .source("my-service/1.0")
    .http_client(http_client)
    .build()?;
# let _ = client;
# Ok(())
# }
```

A caller-provided reqwest client owns its connection pooling, proxy, certificate, and timeout settings. `ClientBuilder::timeout` does not rebuild that client.

## Responses and errors

Successful calls return `ApiResponse<T>` with HTTP status, headers, request/log ID, and decoded body. JSON responses with a non-zero top-level `code` become `Error::Api`; non-JSON HTTP failures become `Error::HttpStatus`.

```rust,no_run
use lark_oapi::Error;

# fn report(error: Error) {
match error {
    Error::Api { code, message, request_id } => {
        eprintln!("platform code={code}, message={message}, request_id={request_id:?}");
    }
    Error::HttpStatus { status, body } => {
        eprintln!("http status={status}, body={body}");
    }
    Error::Http(error) => eprintln!("network error={error}"),
    other => eprintln!("sdk error={other}"),
}
# }
```

Preserve `request_id` for support diagnostics and never log complete access tokens.

## Logging and security

The SDK uses `tracing` for request method/path diagnostics and does not log App Secrets, authorization headers, or cached token values. Applications should install their own subscriber and secret-management policy.

Production recommendations:

- store App Secrets, App Tickets, and one-click registration results in a secret manager;
- do not commit `.env` files or credential-bearing logs;
- validate externally supplied file sizes, MIME types, and endpoint choices;
- encrypt and rotate user tokens;
- never disable event signature verification in production.

## Capability status

| Capability | 0.1.0 |
|---|---:|
| Configurable API base URL, HTTP client, timeout, headers, source | ✓ |
| Self-built app/tenant tokens | ✓ |
| Marketplace App Ticket / Tenant Key flow | ✓ |
| User and explicit tokens | ✓ |
| Pluggable async token cache | ✓ |
| Invalid-token retry | ✓ |
| JSON / form / bytes / multipart | ✓ |
| Generic access to every OpenAPI endpoint | ✓ |
| Typed `im.v1.message.create` | ✓ |
| Event signature verification/decryption/parsing | ✓ |
| One-click registration, presets, add-ons, domain switch | ✓ |
| Full generated typed service coverage | in progress |
| WebSocket long connection | planned |
| Client assertion OAuth | planned |

See [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md) for a detailed official-SDK comparison.

## Development and validation

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features --locked
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
```

The exact tested toolchain, commands, results, and limitations are recorded in:

**[docs/VALIDATION.md](docs/VALIDATION.md)**

## License

MIT. See [LICENSE](LICENSE) and [NOTICE](NOTICE).

This is a community implementation and is not currently an official Feishu/Lark-maintained SDK. Feishu, Lark, and related marks belong to their respective owners.
