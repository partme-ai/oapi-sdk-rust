# oapi-sdk-rust

Async Rust SDK for the Feishu/Lark Open Platform, inspired by the official Go, Python and Java SDKs.

The first release establishes a production-oriented transport core and includes a complete Rust implementation of the one-click application registration flow.

- Async reqwest/Tokio client
- Self-built token management and marketplace tokens with caller-provided app tickets
- Pluggable token cache and HTTP client
- Automatic retry after managed-token invalidation
- JSON, form, bytes and multipart bodies
- Generic access to every OpenAPI endpoint
- Typed IM message API
- Event signature verification and AES-256-CBC decryption
- One-click application registration with QR-code presets and add-ons
- Runnable `create-an-app-in-one-click-rust` example

Chinese documentation: [README.zh-CN.md](README.zh-CN.md)

## One-click app creation

```bash
cd examples/create-an-app-in-one-click-rust
cp .env.example .env
cargo run
```

Detailed guide: [docs/create-an-app-in-one-click-rust.md](docs/create-an-app-in-one-click-rust.md)

Build and test report: [docs/VALIDATION.md](docs/VALIDATION.md)

## Generic OpenAPI request

```rust,no_run
use lark_oapi::{ApiRequest, Client};
use serde_json::Value;

# async fn run() -> lark_oapi::Result<()> {
let client = Client::builder("cli_xxx", "app_secret").build()?;
let request = ApiRequest::get("/open-apis/contact/v3/users/:user_id")
    .path_param("user_id", "ou_xxx")
    .query("user_id_type", "open_id")
    .tenant_access_token();
let response = client.execute::<Value>(request).await?;
println!("{}", response.body);
# Ok(())
# }
```

## Status

Version `0.1.0` provides the stable SDK core and the required registration scenario. Typed service coverage will expand through generated modules; the generic request layer is available immediately for APIs not yet generated.

## License

MIT
