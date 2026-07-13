# 使用 Rust 一键创建飞书 / Lark 应用

本文说明如何使用 `oapi-sdk-rust` 完成飞书开放平台“应用一键创建”流程。实现对齐官方 Java、Go、Python SDK 的 `registration` 场景，并提供可直接运行的 Rust 工程：

```text
examples/create-an-app-in-one-click-rust
```

> 该流程会创建新应用，或在传入已有 App ID 时发起增量配置确认。它不是普通的 Tenant Access Token OpenAPI，而是账户域上的设备码注册流程。

## 1. 能力范围

Rust 实现支持：

- 启动 `PersonalAgent` 应用注册会话；
- 生成并回调验证 URL；
- 在终端渲染二维码；
- 预填应用名称、描述和 1–6 个头像候选；
- 增量声明应用/用户身份权限、应用/用户身份事件和回调；
- `createOnly` 仅创建模式；
- 传入已有 `clientID`，让用户确认增量配置变更；
- `preset=false` 最小基础模板；
- 处理 `authorization_pending`、`slow_down`、`access_denied`、`expired_token`；
- 检测 Lark 租户并自动切换账户域；
- 返回 `client_id`、`client_secret` 和扫码用户信息；
- 在 `Debug` 输出中自动隐藏 `client_secret`。

## 2. 运行条件

- Rust stable，最低支持版本为 `1.85`；
- 可访问 `https://accounts.feishu.cn`；
- Lark 租户还需要访问 `https://accounts.larksuite.com`；
- 已安装并登录飞书或 Lark 客户端；
- 当前登录用户有权创建应用，或有权确认目标已有应用的配置变更；
- 终端支持 Unicode 时可显示字符二维码；无法显示时仍可复制 URL 到浏览器打开。

示例不要求预先提供 App ID 或 App Secret，因为它的目标就是创建应用并获得凭据。

## 3. 下载并运行

```bash
git clone https://github.com/partme-ai/oapi-sdk-rust.git
cd oapi-sdk-rust/examples/create-an-app-in-one-click-rust
cp .env.example .env
cargo run
```

程序启动后会：

1. 请求账户域创建注册会话；
2. 输出二维码有效期；
3. 在终端显示二维码和原始 URL；
4. 等待用户扫码并确认；
5. 输出轮询状态；
6. 成功后打印应用凭据。

典型输出：

```text
请使用飞书或 Lark 扫描二维码（600 秒内有效）：

████████████████████████
██ ▄▄▄▄▄ █▀▄█ ▄▄▄▄▄ ██
...

无法扫码时可直接打开：
https://accounts.feishu.cn/... 

等待用户确认应用创建…

应用创建成功：
FEISHU_APP_ID=cli_xxx
FEISHU_APP_SECRET=xxxxxxxx
CREATOR_OPEN_ID=ou_xxx
TENANT_BRAND=feishu
```

## 4. 环境变量

编辑示例目录下的 `.env`：

```dotenv
APP_NAME={user}的 Rust 智能体
APP_DESCRIPTION=通过 oapi-sdk-rust 一键创建
APP_AVATAR_URL=https://example.com/avatar.png
FEISHU_APP_SOURCE=create-an-app-in-one-click-rust
CREATE_ONLY=true
EXISTING_APP_ID=
FEISHU_ACCOUNT_DOMAIN=https://accounts.feishu.cn
LARK_ACCOUNT_DOMAIN=https://accounts.larksuite.com
```

| 变量 | 必须 | 默认值 | 说明 |
|---|---:|---|---|
| `APP_NAME` | 否 | `{user}的 Rust 智能体` | 创建页预填应用名称，支持 `{user}` 占位符 |
| `APP_DESCRIPTION` | 否 | `通过 oapi-sdk-rust 一键创建` | 创建页预填应用描述，支持 `{user}` 占位符 |
| `APP_AVATAR_URL` | 否 | 空 | 单个头像 URL；SDK API 本身支持最多 6 个候选头像 |
| `FEISHU_APP_SOURCE` | 否 | `create-an-app-in-one-click-rust` | 追加到二维码 `source=rust-sdk/{source}` |
| `CREATE_ONLY` | 否 | `true` | 为 `true` 时页面只允许创建新应用 |
| `EXISTING_APP_ID` | 否 | 空 | 已有应用 App ID，通常以 `cli_` 开头 |
| `FEISHU_ACCOUNT_DOMAIN` | 否 | `https://accounts.feishu.cn` | 飞书账户域；测试或私有环境可覆盖 |
| `LARK_ACCOUNT_DOMAIN` | 否 | `https://accounts.larksuite.com` | 检测到 Lark 租户后使用的账户域 |

`.env` 已被 `.gitignore` 排除。仍应确认自己的全局 Git 配置和其他自动备份工具不会上传该文件。

## 5. 示例默认申请的增量能力

示例通过 `addons` 声明：

```text
应用身份权限：im:message:send_as_bot
应用身份事件：im.message.receive_v1
回调：card.action.trigger
```

这些配置会显示在用户打开验证链接后的确认页面中，只有用户确认后才生效。

### Addons 编码

SDK 使用与官方实现一致的编码流程：

1. 校验结构和字符串非空；
2. 序列化为紧凑 JSON；
3. 使用 gzip 压缩；
4. 使用 URL-safe Base64 编码；
5. 去掉 `=` padding；
6. 写入二维码 URL 的 `addons` 查询参数。

例如逻辑结构：

```json
{
  "scopes": {
    "tenant": ["im:message:send_as_bot"],
    "user": ["calendar:calendar:read"]
  },
  "events": {
    "items": {
      "tenant": ["im.message.receive_v1"]
    }
  },
  "callbacks": {
    "items": ["card.action.trigger"]
  }
}
```

SDK 只校验结构和非空值，不会在线查询权限点、事件名或回调名是否存在。名称写错时，最终页面或平台服务可能拒绝配置。

## 6. 协议流程

### 6.1 Begin

SDK 向飞书账户域发起表单请求：

```http
POST https://accounts.feishu.cn/oauth/v1/app/registration
Content-Type: application/x-www-form-urlencoded

action=begin&archetype=PersonalAgent&auth_method=client_secret&request_user_info=open_id
```

预期返回字段：

| 字段 | 说明 |
|---|---|
| `device_code` | 后续轮询使用的设备码 |
| `verification_uri_complete` | 已包含会话参数的完整验证 URL |
| `verification_uri` | 基础验证 URL，当前实现主要使用 complete URL |
| `user_code` | 用户码，平台可能返回 |
| `interval` | 建议轮询间隔；缺失或小于等于 0 时使用 5 秒 |
| `expire_in` | 会话有效期；缺失或小于等于 0 时使用 600 秒 |

如果 `device_code` 或 `verification_uri_complete` 缺失，SDK 返回 `RegistrationError::InvalidResponse`。

### 6.2 构建二维码 URL

SDK 保留服务端已有查询参数，并追加：

```text
from=sdk
tp=sdk
source=rust-sdk/{source}
avatar=...
name=...
desc=...
addons=...
createOnly=true
clientID=cli_xxx
```

规则：

- `avatar` 可重复出现 1–6 次，顺序保持不变，第一个为默认候选；
- `name` 和 `desc` 支持 `{user}` 占位符；
- SDK 自动进行 URL Encode，调用方传原始字符串；
- `createOnly=true` 时页面只允许创建应用；
- `clientID` 用于已有应用增量变更；当页面处于 create-only 模式时，平台可能忽略已有 App ID。

### 6.3 Poll

```http
POST https://accounts.feishu.cn/oauth/v1/app/registration
Content-Type: application/x-www-form-urlencoded

action=poll&device_code=...
```

状态处理：

| 服务端结果 | SDK 行为 |
|---|---|
| `authorization_pending` | 发出 `RegistrationStatus::Polling`，按间隔继续轮询 |
| `slow_down` | 间隔增加 5 秒，发出 `SlowDown { interval }` |
| `access_denied` | 返回 `RegistrationError::AccessDenied` |
| `expired_token` | 返回 `RegistrationError::Expired` |
| 本地到达有效期 | 返回 `RegistrationError::Expired` |
| `tenant_brand=lark` | 切换到 Lark 账户域，发出 `DomainSwitched` 并立即继续 |
| `client_id` + `client_secret` | 返回成功结果 |
| 其他非空 `error` | 返回 `RegistrationError::Service` |
| 无凭据且无错误 | 为兼容官方 Go 行为，继续轮询 |

轮询 Future 被上层任务取消或丢弃时，Tokio 会停止后续操作；Rust API 不额外定义 Java 式线程中断 `abort` 错误码。

## 7. 在自己的项目中调用

### 7.1 一步式 API

```rust,no_run
use lark_oapi::registration::{
    register_app, AppAddons, AppAddonsCallbacks, AppAddonsEvents,
    AppAddonsScopes, AppPreset, RegistrationOptions, RegistrationStatus,
};

# async fn run() -> Result<(), Box<dyn std::error::Error>> {
let options = RegistrationOptions::new()
    .source("my-agent")
    .app_preset(
        AppPreset::new()
            .avatars([
                "https://example.com/a.png",
                "https://example.com/b.png",
            ])
            .name("{user}的智能体")
            .description("由 Rust 创建"),
    )
    .addons(AppAddons {
        preset: None,
        scopes: AppAddonsScopes {
            tenant: vec!["im:message:send_as_bot".into()],
            user: vec![],
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

let result = register_app(
    options,
    |qr| println!("{} 秒内打开：{}", qr.expire_in, qr.url),
    |status| match status {
        RegistrationStatus::Polling => println!("等待确认"),
        RegistrationStatus::SlowDown { interval } => {
            println!("新的轮询间隔：{interval} 秒")
        }
        RegistrationStatus::DomainSwitched => println!("切换到 Lark 域"),
    },
)
.await?;

println!("client_id={}", result.client_id);
// 只将 client_secret 写入密钥管理系统，不要写普通业务日志。
# Ok(())
# }
```

### 7.2 两阶段 API

两阶段 API 适合 Web 页面、桌面应用或需要自行控制二维码呈现的场景：

```rust,no_run
use lark_oapi::registration::{begin_registration, RegistrationOptions};

# async fn run() -> Result<(), Box<dyn std::error::Error>> {
let session = begin_registration(RegistrationOptions::new()).await?;

// 将 URL 转为二维码、返回给前端或在桌面窗口显示。
println!("二维码 URL：{}", session.qr_code().url);
println!("有效期：{} 秒", session.qr_code().expire_in);

let result = session
    .wait_with_status(|status| println!("{status:?}"))
    .await?;

println!("client_id={}", result.client_id);
# Ok(())
# }
```

## 8. 自定义权限、事件和回调

### 8.1 默认基础模板 + 增量配置

```rust,no_run
use lark_oapi::registration::{
    AppAddons, AppAddonsCallbacks, AppAddonsEvents, AppAddonsScopes,
    RegistrationOptions,
};

let options = RegistrationOptions::new().addons(AppAddons {
    preset: None,
    scopes: AppAddonsScopes {
        tenant: vec![
            "im:message:send_as_bot".into(),
            "drive:drive.metadata:readonly".into(),
        ],
        user: vec!["calendar:calendar:read".into()],
    },
    events: AppAddonsEvents {
        tenant: vec!["im.message.receive_v1".into()],
        user: vec!["calendar.calendar.event.changed_v4".into()],
    },
    callbacks: AppAddonsCallbacks {
        items: vec!["card.action.trigger".into()],
    },
});
# let _ = options;
```

### 8.2 最小基础模板

```rust,no_run
use lark_oapi::registration::{AppAddons, RegistrationOptions};

let options = RegistrationOptions::new().addons(AppAddons {
    preset: Some(false),
    ..Default::default()
});
# let _ = options;
```

`preset: Some(false)` 是唯一允许所有增量列表为空的情况。

### 8.3 更新已有应用

```rust,no_run
use lark_oapi::registration::{AppAddons, AppAddonsScopes, RegistrationOptions};

let options = RegistrationOptions::new()
    .app_id("cli_xxx")
    .addons(AppAddons {
        scopes: AppAddonsScopes {
            tenant: vec!["drive:drive.metadata:readonly".into()],
            user: vec![],
        },
        ..Default::default()
    });
# let _ = options;
```

用户打开验证页面后会看到增量配置 diff，并决定是否确认。

## 9. Rust 参数说明

### RegistrationOptions

| 字段 / Builder | 类型 | 必须 | 默认值 | 说明 |
|---|---|---:|---|---|
| `source(...)` | `String` | 否 | 无 | URL 中为 `rust-sdk/{source}` |
| `domains(...)` | 两个 `String` | 否 | 官方飞书/Lark 账户域 | 测试、代理或私有环境覆盖 |
| `app_preset(...)` | `AppPreset` | 否 | 无 | 预填名称、描述和头像 |
| `addons(...)` | `AppAddons` | 否 | 无 | 增量权限、事件和回调 |
| `create_only(...)` | `bool` | 否 | `false` | 只允许创建新应用 |
| `app_id(...)` | `String` | 否 | 无 | 已有应用 App ID |

### AppPreset

| 字段 / Builder | 约束 |
|---|---|
| `avatar(...)` | 一个非空 URL 字符串 |
| `avatars(...)` | 1–6 个非空 URL，保持顺序 |
| `name(...)` | 可选，支持 `{user}` |
| `description(...)` | 可选，支持 `{user}` |

### AppAddons

| 字段 | 示例 |
|---|---|
| `preset` | `None`、`Some(true)` 或 `Some(false)` |
| `scopes.tenant` | `im:message:send_as_bot` |
| `scopes.user` | `calendar:calendar:read` |
| `events.tenant` | `im.message.receive_v1` |
| `events.user` | `calendar.calendar.event.changed_v4` |
| `callbacks.items` | `card.action.trigger` |

## 10. 返回值、状态和错误

### RegisterAppResult

| 字段 | 类型 | 说明 |
|---|---|---|
| `client_id` | `String` | 新建或确认后的 App ID |
| `client_secret` | `String` | App Secret；`Debug` 输出会显示 `<redacted>` |
| `user_info` | `Option<RegisteredUserInfo>` | 扫码用户信息 |
| `user_info.open_id` | `Option<String>` | 用户 `open_id` |
| `user_info.tenant_brand` | `Option<String>` | 通常为 `feishu` 或 `lark` |

### RegistrationStatus

| 状态 | 说明 |
|---|---|
| `Polling` | 等待用户确认 |
| `SlowDown { interval }` | 服务端要求放慢轮询，单位为秒 |
| `DomainSwitched` | 检测到 Lark 租户并切换账户域 |

### RegistrationError

| Variant | 说明 |
|---|---|
| `InvalidArgument` | 参数形状、空值、头像数量或 Addons 不合法 |
| `Network` | HTTP 连接、超时或传输错误 |
| `Url` | 域名或验证 URL 无法解析 |
| `Json` | 服务端 JSON 无法解析 |
| `Io` | Addons gzip 编码失败 |
| `InvalidResponse` | 缺少必要字段或响应不符合协议 |
| `AccessDenied` | 用户拒绝 |
| `Expired` | 服务端报告过期或本地轮询超时 |
| `Service` | 其他平台错误码 |

错误处理示例：

```rust,no_run
use lark_oapi::registration::RegistrationError;

# fn report(error: RegistrationError) {
match error {
    RegistrationError::AccessDenied { code, description } => {
        eprintln!("用户拒绝：{code} {description}");
    }
    RegistrationError::Expired { code, description } => {
        eprintln!("二维码过期：{code} {description}");
    }
    RegistrationError::Service { code, description } => {
        eprintln!("平台错误：{code} {description}");
    }
    other => eprintln!("注册失败：{other}"),
}
# }
```

## 11. 凭据安全

示例为了完成官方教程，会在用户明确确认后输出 App Secret。生产系统应立即：

- 写入云 KMS、Vault、Kubernetes Secret 或等价密钥管理系统；
- 限制读取权限并启用审计；
- 从普通日志、错误上报和终端录屏中移除 Secret；
- 不把 Secret 写进源代码、镜像层、CI 参数明文或 Git 历史；
- 在凭据疑似泄露时立即到开发者后台重置；
- 对创建的应用记录创建者、租户、时间和业务用途。

`RegisterAppResult` 的 `Debug` 实现会隐藏 Secret，但直接访问 `client_secret` 后由业务代码负责安全处理。

## 12. 常见问题

### 二维码生成成功，但一直显示等待确认

检查：

- 用户是否使用正确的飞书/Lark 客户端登录；
- 用户是否完成页面中的最终确认，而不只是扫码；
- 终端程序是否仍在运行；
- 出口代理是否允许持续访问账户域；
- 是否已超过 `expire_in`。

### 出现 `slow_down`

这是协议定义的正常状态。SDK 会自动增加 5 秒轮询间隔，不应自行启动并行轮询。

### Lark 用户扫码后切换域

当轮询响应包含 `tenant_brand=lark` 时，SDK 会发出 `DomainSwitched` 并切换至 `lark_domain`。确保网络策略同时允许飞书和 Lark 账户域。

### Addons 报必须包含配置项

平台默认模板模式下，Addons 至少要包含一个权限、事件或回调。只需要最小基础模板且没有增量项时设置：

```rust
AppAddons {
    preset: Some(false),
    ..Default::default()
}
```

### 可以通过 Addons 设置 Event URL 或 Encrypt Key 吗

不可以。Addons 只承载公开的权限、事件名称和回调名称。敏感开发配置应在应用创建后通过开发者后台或应用配置 OpenAPI 设置。

### 可以不输出 App Secret 吗

可以。示例为了教程体验才输出；业务代码可在获得结果后直接写入密钥系统，并只向终端显示 App ID。

## 13. 实际验收清单

执行真实端到端验证时建议逐项记录：

- [ ] `cargo run` 成功启动；
- [ ] 终端二维码或 URL 可打开；
- [ ] 名称、描述和头像正确预填；
- [ ] 权限、事件、回调与配置一致；
- [ ] 用户能完成创建；
- [ ] `Polling` 状态正常出现；
- [ ] 如使用 Lark，出现 `DomainSwitched`；
- [ ] 返回 `client_id` 和非空 `client_secret`；
- [ ] `tenant_brand` 与实际产品一致；
- [ ] Secret 被写入安全存储；
- [ ] 新应用可使用返回的凭据获取 Tenant Access Token；
- [ ] 创建后的机器人能按授权发送测试消息。

仓库的自动化测试不会执行真实扫码和应用创建，因为该操作需要真人确认并会修改真实飞书/Lark 租户。构建测试范围见 [VALIDATION.md](VALIDATION.md)。
