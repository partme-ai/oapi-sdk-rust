[English](README.md) | **简体中文**

# 飞书 / Lark 开放平台 Rust SDK

[![CI](https://github.com/partme-ai/oapi-sdk-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/partme-ai/oapi-sdk-rust/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue)](rust-toolchain.toml)
[![License](https://img.shields.io/badge/license-MIT-green)](LICENSE)

`oapi-sdk-rust` 是面向飞书 / Lark 开放平台的异步 Rust SDK。项目参考官方 Go、Python、Java SDK 的鉴权、请求、事件和场景能力，使用 Rust 风格的 builder、enum、trait 与 async API 实现。

SDK 旨在减少调用服务端 API 时的重复工作，包括：获取和缓存访问令牌、构造请求、解析平台错误、上传下载文件、事件签名验证、加密回调解密，以及应用一键创建。

> 当前版本：`0.1.0`。稳定内核、通用 OpenAPI 调用、`im.v1.message.create` 类型化接口、事件回调和应用一键创建已经可用；完整业务 API 类型生成仍在持续建设。尚未类型化的接口可以立即通过 `ApiRequest` 调用。

## 文档导航

- [应用一键创建完整指南](docs/create-an-app-in-one-click-rust.md)
- [构建与测试报告](docs/VALIDATION.md)
- [架构说明](docs/ARCHITECTURE.md)
- [官方 SDK 能力兼容矩阵](docs/COMPATIBILITY.md)
- [开发路线图](docs/ROADMAP.md)
- [贡献指南](CONTRIBUTING.md)

## 问题反馈

请在 [GitHub Issues](https://github.com/partme-ai/oapi-sdk-rust/issues) 提交缺陷、接口覆盖需求或兼容性问题。报告问题时建议附上：

- `lark-oapi` 版本与 Rust 版本；
- 目标 API 路径和鉴权类型；
- 平台返回的 `code`、`msg` 与 `request_id`；
- 可复现的最小代码；
- 已脱敏的请求与响应信息。

不要在 Issue 中提交 App Secret、Access Token、App Ticket、Encrypt Key 或完整 Authorization Header。

## 运行环境

- Rust `1.85` 及以上；
- Tokio 异步运行时；
- 默认使用 rustls TLS，不依赖系统 OpenSSL；
- 支持 Linux、macOS 和 Windows；
- 访问飞书需要连通 `open.feishu.cn` 和 `accounts.feishu.cn`；
- 访问 Lark 时可将 API 域设置为 `open.larksuite.com`，一键创建流程会在检测到 Lark 租户后自动切换至 `accounts.larksuite.com`。

## 安装

### 使用 Git 依赖

在 crates.io 正式发布前：

```toml
[dependencies]
lark-oapi = { git = "https://github.com/partme-ai/oapi-sdk-rust", features = ["registration", "events"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

也可以使用 Cargo 命令：

```bash
cargo add lark-oapi --git https://github.com/partme-ai/oapi-sdk-rust --features registration,events
cargo add tokio --features macros,rt-multi-thread
```

### 本地开发

```toml
[dependencies]
lark-oapi = { path = "../oapi-sdk-rust" }
```

### Feature 开关

| Feature | 默认启用 | 作用 |
|---|---:|---|
| `rustls-tls` | 是 | 使用 rustls TLS |
| `native-tls` | 否 | 使用系统 Native TLS；不要与 `rustls-tls` 同时启用 |
| `registration` | 是 | 应用一键创建与增量配置编码 |
| `events` | 是 | 事件签名验证、AES 解密和回调解析 |

最小 HTTP 客户端：

```toml
lark-oapi = { git = "https://github.com/partme-ai/oapi-sdk-rust", default-features = false, features = ["rustls-tls"] }
```

使用系统 TLS：

```toml
lark-oapi = { git = "https://github.com/partme-ai/oapi-sdk-rust", default-features = false, features = ["native-tls", "registration", "events"] }
```

## 术语说明

- **飞书 / Feishu**：Lark 在中国市场的产品名称，使用独立的开放平台与账户域名。
- **Lark**：面向海外市场的产品名称，OpenAPI 默认域为 `https://open.larksuite.com`。
- **企业自建应用**：仅在创建该应用的企业内安装使用，SDK 可通过 App ID 和 App Secret 自动获取 App/Tenant Access Token。
- **应用商店应用**：面向多个企业分发。获取 App Access Token 时需要 App Ticket，获取 Tenant Access Token 时需要 Tenant Key。
- **App Access Token**：应用身份访问令牌。
- **Tenant Access Token**：应用在某个租户中的访问令牌，大多数机器人和企业 API 使用此令牌。
- **User Access Token**：用户身份访问令牌，必须由调用方显式提供。
- **Request ID / Log ID**：平台返回的请求追踪标识，排查问题时应保留。

## 快速开始

### 创建企业自建应用客户端

App ID 和 App Secret 可在开发者后台的“凭证与基础信息”中获取。

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

SDK 默认自动获取并缓存 App/Tenant Access Token。调用业务 API 时通常不需要自行申请这两类令牌。

### 调用类型化服务端 API

当前首个类型化业务接口为发送消息：

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

`receive_id_type` 常见取值包括 `open_id`、`user_id`、`union_id`、`email` 和 `chat_id`。

### 调用尚未类型化的 OpenAPI

通用请求层支持任意 HTTP 方法、路径参数、重复查询参数、Header 和不同请求体：

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

路径变量同时支持 `:user_id` 和 `{user_id}`。SDK 会对路径变量进行百分号编码。

### 请求体

#### JSON

```rust,no_run
use lark_oapi::ApiRequest;

# fn build() -> lark_oapi::Result<ApiRequest> {
ApiRequest::post("/open-apis/example/v1/resources")
    .tenant_access_token()
    .json(&serde_json::json!({"name": "demo"}))
# }
```

#### Form

```rust,no_run
use lark_oapi::ApiRequest;

let request = ApiRequest::post("https://example.com/token")
    .form([("grant_type", "client_credentials")]);
```

#### Multipart 文件上传

```rust,no_run
use bytes::Bytes;
use lark_oapi::{ApiRequest, MultipartField};

let request = ApiRequest::post("/open-apis/im/v1/files")
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
```

#### 原始字节下载

```rust,no_run
use lark_oapi::{ApiRequest, Client};

# async fn run(client: &Client) -> lark_oapi::Result<()> {
let response = client
    .execute_bytes(
        ApiRequest::get("/open-apis/drive/v1/medias/:file_token/download")
            .path_param("file_token", "file_xxx")
            .tenant_access_token(),
    )
    .await?;

std::fs::write("download.bin", &response.body)
    .map_err(|error| lark_oapi::Error::InvalidResponse(error.to_string()))?;
# Ok(())
# }
```

## 鉴权与 Token

### 企业自建应用

调用 `.app_access_token()` 或 `.tenant_access_token()` 时，SDK 会：

1. 查询 `TokenCache`；
2. 缓存未命中时调用官方鉴权接口；
3. 按服务端过期时间提前预留安全窗口；
4. 在平台返回 Token 失效代码时清理缓存并自动重试一次。

```rust,no_run
use lark_oapi::ApiRequest;

let app_request = ApiRequest::get("/open-apis/example/app-api")
    .app_access_token();
let tenant_request = ApiRequest::get("/open-apis/example/tenant-api")
    .tenant_access_token();
```

### User Access Token

SDK 不会替用户执行 OAuth 授权。使用用户身份 API 时显式传入：

```rust,no_run
use lark_oapi::ApiRequest;

let request = ApiRequest::get("/open-apis/calendar/v4/calendars")
    .user_access_token("u-xxxxxxxx");
```

### 显式 App/Tenant Token

关闭自动缓存或从外部令牌服务获得 Token 时：

```rust,no_run
use lark_oapi::{AccessTokenType, ApiRequest};

let request = ApiRequest::get("/open-apis/contact/v3/users")
    .explicit_access_token(AccessTokenType::Tenant, "t-xxxxxxxx");
```

仅使用显式 Token 时，构建客户端的 `app_secret` 可以为空字符串；只有 SDK 需要自动获取 App/Tenant Token 时才要求 App Secret。

### 应用商店应用

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

当前版本不持久化 App Ticket；调用方应安全保存平台推送的 App Ticket，并在请求中提供。

### 自定义 TokenCache

`TokenCache` 是异步 trait，可接入 Redis、数据库或分布式缓存：

```rust,no_run
use std::sync::Arc;
use lark_oapi::{Client, MemoryTokenCache};

# fn build() -> lark_oapi::Result<()> {
let client = Client::builder("cli_xxx", "app_secret")
    .token_cache(Arc::new(MemoryTokenCache::default()))
    .build()?;
# let _ = client;
# Ok(())
# }
```

生产集群应使用共享缓存，避免每个实例重复申请令牌。

## 应用一键创建

SDK 的 `registration` 模块实现与官方 Java、Go、Python SDK 对齐的应用注册流程。它使用设备码协议生成验证链接，用户在飞书/Lark 中确认后返回 App ID 和 App Secret。

### 运行官方风格示例

```bash
git clone https://github.com/partme-ai/oapi-sdk-rust.git
cd oapi-sdk-rust/examples/create-an-app-in-one-click-rust
cp .env.example .env
cargo run
```

示例会在终端渲染二维码，并在创建成功后输出：

```text
FEISHU_APP_ID=cli_xxx
FEISHU_APP_SECRET=xxxxxxxx
CREATOR_OPEN_ID=ou_xxx
TENANT_BRAND=feishu
```

完整操作、环境变量、协议流程、故障排查和验收清单见：

**[docs/create-an-app-in-one-click-rust.md](docs/create-an-app-in-one-click-rust.md)**

### SDK 调用

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
            .name("{user}的智能体")
            .description("由 Rust SDK 创建"),
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
    |qr| println!("请扫码或打开：{}，{} 秒内有效", qr.url, qr.expire_in),
    |status| match status {
        RegistrationStatus::Polling => println!("等待用户确认"),
        RegistrationStatus::SlowDown { interval } => {
            println!("轮询间隔调整为 {interval} 秒")
        }
        RegistrationStatus::DomainSwitched => println!("已切换到 Lark 账户域"),
    },
)
.await?;

println!("app_id={}", credentials.client_id);
# Ok(())
# }
```

### 创建、更新与最小模板

```rust,no_run
use lark_oapi::registration::{AppAddons, AppAddonsScopes, RegistrationOptions};

// 只允许创建新应用。
let create = RegistrationOptions::new()
    .create_only(true)
    .addons(AppAddons {
        scopes: AppAddonsScopes {
            tenant: vec!["im:message:send_as_bot".into()],
            user: vec![],
        },
        ..Default::default()
    });

// 让用户确认已有应用的增量配置变更。
let update = RegistrationOptions::new()
    .app_id("cli_xxx")
    .addons(AppAddons {
        scopes: AppAddonsScopes {
            tenant: vec!["drive:drive.metadata:readonly".into()],
            user: vec![],
        },
        ..Default::default()
    });

// 使用最小基础模板；preset=false 时允许不传任何增量项。
let minimal = RegistrationOptions::new().addons(AppAddons {
    preset: Some(false),
    ..Default::default()
});
# let _ = (create, update, minimal);
```

注意：

- `addons` 只做增量叠加，不能删除平台基础模板中的配置；
- `preset: None` 或 `Some(true)` 使用平台默认基础模板；
- `preset: Some(false)` 使用最小基础模板；
- 支持应用/用户身份权限、应用/用户身份事件和回调；
- Event URL、Encrypt Key、Security 配置等敏感信息不能通过 `addons` 设置；
- SDK 校验结构和非空值，不校验权限点、事件名或回调名是否真实存在。

## 事件订阅与回调

开发者后台中可配置 Verification Token 和 Encrypt Key。`EventParser` 会按顺序执行签名验证、AES-256-CBC 解密、Verification Token 校验和事件解析。

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
    ParsedEvent::Event {
        event_type,
        header,
        event,
        ..
    } => {
        println!("event_type={event_type}");
        println!("event_id={:?}", header.and_then(|value| value.event_id));
        println!("payload={event}");
        r#"{"code":0}"#.to_owned()
    }
};
# Ok(response)
# }
```

`skip_signature_verification(true)` 仅用于受控测试，不应在生产环境开启。

## Client 高级配置

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

| 配置 | 默认值 | 说明 |
|---|---|---|
| `base_url` | `https://open.feishu.cn` | OpenAPI 基地址；Lark 使用 `https://open.larksuite.com` |
| `oauth_base_url` | 未设置 | 预留的 OAuth 基地址配置 |
| `timeout` | 30 秒 | SDK 创建的 reqwest Client 的总请求超时 |
| `app_type` | `SelfBuilt` | `SelfBuilt` 或 `Marketplace` |
| `enable_token_cache` | `true` | 是否读取和写入 TokenCache |
| `token_cache` | `MemoryTokenCache` | 可替换的异步令牌缓存 |
| `default_header` | 无 | 添加到每次 OpenAPI 请求的 Header |
| `source` | 无 | 附加到 User-Agent 的来源标识 |
| `http_client` | SDK 创建 | 使用调用方配置的 reqwest Client |

传入自定义 `reqwest::Client` 后，连接池、代理、证书和超时应由调用方配置；`ClientBuilder::timeout` 不会重建该 Client。

## 响应与错误处理

### 响应元数据

所有成功请求返回 `ApiResponse<T>`：

- `status`：HTTP 状态码；
- `headers`：响应 Header；
- `request_id`：从 `X-Tt-Logid`、`X-Request-Id` 等 Header 提取的追踪标识；
- `body`：JSON 反序列化结果或原始字节。

### 错误类型

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
    Error::InvalidParameter(message) => eprintln!("invalid input={message}"),
    other => eprintln!("sdk error={other}"),
}
# }
```

对于 JSON API，顶层非零 `code` 会转换为 `Error::Api`。非 JSON 的失败响应会转换为 `Error::HttpStatus`。诊断时优先记录 `request_id`，不要记录完整 Access Token。

## 日志与安全

SDK 使用 `tracing` 输出请求方法和路径，不输出 Authorization Header、App Secret 或缓存 Token。调用方可以安装自己的 subscriber：

```rust,no_run
# fn init() {
tracing_subscriber::fmt()
    .with_env_filter("lark_oapi=debug")
    .init();
# }
```

`tracing-subscriber` 由业务项目自行添加。生产环境还应：

- 使用密钥管理服务保存 App Secret、App Ticket 和一键创建结果；
- 禁止将 `.env` 和凭据日志提交到 Git；
- 对外部输入的文件大小、MIME 类型和目标 API 做限制；
- 为用户令牌建立加密存储、刷新和撤销流程；
- 不在生产环境跳过事件签名校验。

## 已实现范围与兼容性

| 能力 | 0.1.0 |
|---|---:|
| 可配置 API 域、HTTP Client、超时、Header、Source | ✓ |
| 企业自建应用 App/Tenant Token | ✓ |
| 应用商店应用 App Ticket / Tenant Key | ✓ |
| User Token 与显式 Token | ✓ |
| 可替换 TokenCache | ✓ |
| Token 失效自动重试 | ✓ |
| JSON / Form / Bytes / Multipart | ✓ |
| 任意 OpenAPI 通用调用 | ✓ |
| `im.v1.message.create` 类型化 API | ✓ |
| 事件验签、解密和解析 | ✓ |
| 应用一键创建、预设、Addons、域切换 | ✓ |
| 完整类型化业务 API | 建设中 |
| WebSocket 长连接 | 规划中 |
| Client Assertion OAuth | 规划中 |

更详细的官方 SDK 对照见 [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md)。

## 开发与验证

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features --locked
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
```

当前提交的实际构建结果和限制见：

**[docs/VALIDATION.md](docs/VALIDATION.md)**

## License

MIT。详见 [LICENSE](LICENSE) 与 [NOTICE](NOTICE)。

本项目为社区实现，当前不属于飞书 / Lark 官方维护的 SDK。飞书、Feishu、Lark 及相关标识归其权利人所有。
