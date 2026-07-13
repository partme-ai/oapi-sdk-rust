use std::env;

use anyhow::Result;
use lark_oapi::registration::{
    register_app, AppAddons, AppAddonsCallbacks, AppAddonsEvents, AppAddonsScopes, AppPreset,
    RegistrationOptions, RegistrationStatus,
};
use qrcode::{render::unicode, QrCode};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let mut preset = AppPreset::new()
        .name(env_or("APP_NAME", "{user}的 Rust 智能体"))
        .description(env_or("APP_DESCRIPTION", "通过 oapi-sdk-rust 一键创建"));
    if let Some(avatar) = non_empty_env("APP_AVATAR_URL") {
        preset = preset.avatar(avatar);
    }

    // These additions create the baseline permissions needed by a messaging
    // agent. Add only public scopes/events/callbacks here; sensitive settings
    // should be updated through the application-configuration OpenAPI.
    let addons = AppAddons {
        scopes: AppAddonsScopes {
            tenant: vec!["im:message:send_as_bot".into()],
            user: Vec::new(),
        },
        events: AppAddonsEvents {
            tenant: vec!["im.message.receive_v1".into()],
            user: Vec::new(),
        },
        callbacks: AppAddonsCallbacks {
            items: vec!["card.action.trigger".into()],
        },
        ..Default::default()
    };

    let mut options = RegistrationOptions::new()
        .source(env_or(
            "FEISHU_APP_SOURCE",
            "create-an-app-in-one-click-rust",
        ))
        .app_preset(preset)
        .addons(addons)
        .create_only(bool_env("CREATE_ONLY", true));
    if let Some(app_id) = non_empty_env("EXISTING_APP_ID") {
        options = options.app_id(app_id);
    }

    let result = register_app(
        options,
        |qr| {
            println!(
                "\n请使用飞书或 Lark 扫描二维码（{} 秒内有效）：\n",
                qr.expire_in
            );
            match QrCode::new(qr.url.as_bytes()) {
                Ok(code) => {
                    let rendered = code.render::<unicode::Dense1x2>().quiet_zone(true).build();
                    println!("{rendered}");
                }
                Err(error) => eprintln!("二维码渲染失败：{error}"),
            }
            println!("无法扫码时可直接打开：\n{}\n", qr.url);
        },
        |status| match status {
            RegistrationStatus::Polling => println!("等待用户确认应用创建…"),
            RegistrationStatus::SlowDown { interval } => {
                println!("服务端要求降低轮询频率，新间隔：{interval} 秒")
            }
            RegistrationStatus::DomainSwitched => {
                println!("检测到 Lark 租户，已自动切换到 Lark 账户域")
            }
        },
    )
    .await?;

    println!("\n应用创建成功：");
    println!("FEISHU_APP_ID={}", result.client_id);
    println!("FEISHU_APP_SECRET={}", result.client_secret);
    if let Some(user) = result.user_info {
        if let Some(open_id) = user.open_id {
            println!("CREATOR_OPEN_ID={open_id}");
        }
        if let Some(tenant_brand) = user.tenant_brand {
            println!("TENANT_BRAND={tenant_brand}");
        }
    }
    eprintln!("\n请立即将 APP_SECRET 保存到密钥管理系统，不要提交到 Git。\n");

    Ok(())
}

fn env_or(name: &str, default: &str) -> String {
    non_empty_env(name).unwrap_or_else(|| default.to_owned())
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn bool_env(name: &str, default: bool) -> bool {
    non_empty_env(name)
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(default)
}
