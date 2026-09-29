/// OneAPI 余额/充值代理命令
/// 通过用户名 + 密码登录云端 OneAPI 会话，代理查询余额、测试连接等操作
use serde_json::{json, Value};

/// OneAPI / New API 默认计价单位（1 美元 = 500000 额度）
const DEFAULT_QUOTA_PER_UNIT: f64 = 500_000.0;

/// 构建请求客户端
fn build_client() -> Result<reqwest::Client, String> {
    crate::commands::build_http_client(std::time::Duration::from_secs(15), Some("ClawPanel/0.1"))
}

/// 从 clawpanel.json 读取 oneapi 配置段，返回 (url, username, password)
fn read_oneapi_config() -> Option<(String, String, String)> {
    crate::commands::read_panel_config_value().and_then(|v| {
        let oneapi = v.get("oneapi")?;
        let url = oneapi
            .get("url")?
            .as_str()?
            .trim()
            .trim_end_matches('/')
            .to_string();
        let username = oneapi
            .get("username")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let password = oneapi
            .get("password")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if url.is_empty() || username.is_empty() || password.is_empty() {
            return None;
        }
        Some((url, username, password))
    })
}

/// 提取 OneAPI 会话 Cookie（gin sessions 默认名为 session）
fn extract_session_cookie(resp: &reqwest::Response) -> Option<String> {
    resp.headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .filter_map(|raw| raw.split(';').next().map(str::trim))
        .find(|pair| pair.starts_with("session="))
        .map(|pair| pair.to_string())
}

/// 登录 OneAPI，返回 (会话 Cookie, 用户数字 ID)
async fn oneapi_login(
    client: &reqwest::Client,
    url: &str,
    username: &str,
    password: &str,
) -> Result<(String, Option<i64>), String> {
    let resp = client
        .post(format!("{url}/api/user/login"))
        .json(&json!({ "username": username, "password": password }))
        .send()
        .await
        .map_err(|e| format!("OneAPI 登录请求失败: {e}"))?;

    let cookie = extract_session_cookie(&resp);
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("解析登录响应失败: {e}"))?;

    if body.get("success").and_then(|v| v.as_bool()) == Some(false) {
        let msg = body
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("用户名或密码错误");
        return Err(format!("OneAPI 登录失败: {msg}"));
    }
    let cookie = cookie.ok_or("OneAPI 登录失败：服务器未返回会话")?;

    let data = body.get("data").cloned().unwrap_or(json!({}));
    if data.get("require_2fa").and_then(|v| v.as_bool()) == Some(true) {
        return Err("该账号启用了两步验证，无法在面板中自动查询余额".to_string());
    }
    Ok((cookie, data.get("id").and_then(|v| v.as_i64())))
}

/// 读取 OneAPI 计价配置：返回 (每单位额度, 是否以货币展示)
async fn fetch_quota_config(client: &reqwest::Client, url: &str) -> (f64, bool) {
    let fallback = (DEFAULT_QUOTA_PER_UNIT, true);
    let resp = match client.get(format!("{url}/api/status")).send().await {
        Ok(resp) => resp,
        Err(_) => return fallback,
    };
    let body: Value = match resp.json().await {
        Ok(body) => body,
        Err(_) => return fallback,
    };
    let data = body.get("data").unwrap_or(&body);
    let per_unit = data
        .get("quota_per_unit")
        .and_then(|v| v.as_f64())
        .filter(|n| *n > 0.0)
        .unwrap_or(DEFAULT_QUOTA_PER_UNIT);
    let display = data
        .get("display_in_currency")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    (per_unit, display)
}

/// 登录并拉取 OneAPI 用户信息，返回 (用户数据, 每单位额度, 是否货币展示)
async fn oneapi_fetch_self(
    url: &str,
    username: &str,
    password: &str,
) -> Result<(Value, f64, bool), String> {
    let client = build_client()?;
    let (cookie, user_id) = oneapi_login(&client, url, username, password).await?;
    let (per_unit, display_in_currency) = fetch_quota_config(&client, url).await;

    let mut req = client
        .get(format!("{url}/api/user/self"))
        .header("Cookie", cookie);
    // New API 用会话访问用户接口时仍要求 New-Api-User；OneAPI 会忽略该头
    if let Some(id) = user_id {
        req = req.header("New-Api-User", id.to_string());
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("OneAPI 请求失败: {e}"))?;
    let status = resp.status();
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {e}"))?;

    let ok = status.is_success() && body.get("success").and_then(|v| v.as_bool()) != Some(false);
    if !ok {
        let msg = body
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("未知错误");
        return Err(format!("OneAPI 返回错误 ({}): {msg}", status.as_u16()));
    }

    let data = body.get("data").cloned().unwrap_or(body);
    Ok((data, per_unit, display_in_currency))
}

/// 把 OneAPI 原始额度换算成展示值，语义与官方面板 renderQuota 保持一致
fn render_quota(quota: f64, per_unit: f64, display_in_currency: bool) -> Value {
    if display_in_currency && per_unit > 0.0 {
        json!((quota / per_unit * 100.0).round() / 100.0)
    } else {
        json!(quota.round())
    }
}

/// 获取 OneAPI 用户余额和用量信息
#[tauri::command]
pub async fn oneapi_get_balance() -> Result<Value, String> {
    let (url, username, password) =
        read_oneapi_config().ok_or("请先在钱包页面配置 OneAPI 连接信息")?;
    let (data, per_unit, display) = oneapi_fetch_self(&url, &username, &password).await?;

    let quota = data.get("quota").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let used = data
        .get("used_quota")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    Ok(json!({
        "balance": render_quota(quota, per_unit, display),
        "used_quota": render_quota(used, per_unit, display),
        "display_in_currency": display,
        "username": data.get("username").and_then(|v| v.as_str()).unwrap_or(username.as_str()),
        "email": data.get("email").and_then(|v| v.as_str()).unwrap_or(""),
        "group": data.get("group").and_then(|v| v.as_str()).unwrap_or(""),
    }))
}

/// 获取 OneAPI 配置
#[tauri::command]
pub fn oneapi_get_config() -> Result<Value, String> {
    let cfg = crate::commands::read_panel_config_value().unwrap_or(json!({}));
    let oneapi = cfg.get("oneapi").cloned().unwrap_or(json!({
        "url": "",
        "username": "",
        "password": "",
    }));
    Ok(oneapi)
}

/// 保存 OneAPI 配置到 clawpanel.json
#[tauri::command]
pub fn oneapi_save_config(config: Value) -> Result<(), String> {
    let field = |key: &str| -> String {
        config
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    let section = json!({
        "url": field("url").trim_end_matches('/').to_string(),
        "username": field("username"),
        "password": field("password"),
    });

    let mut cfg = crate::commands::config::read_panel_config().unwrap_or_else(|_| json!({}));
    if !cfg.is_object() {
        cfg = json!({});
    }
    cfg["oneapi"] = section;
    crate::commands::config::write_panel_config(cfg)
}

/// 测试 OneAPI 连接
#[tauri::command]
pub async fn oneapi_test_connection() -> Result<Value, String> {
    let (url, username, password) =
        read_oneapi_config().ok_or("请先在钱包页面配置 OneAPI 连接信息")?;
    let start = std::time::Instant::now();
    match oneapi_fetch_self(&url, &username, &password).await {
        Ok((data, _, _)) => Ok(json!({
            "ok": true,
            "latency_ms": start.elapsed().as_millis(),
            "email": data.get("email").and_then(|v| v.as_str()).unwrap_or(""),
            "username": data.get("username").and_then(|v| v.as_str()).unwrap_or(username.as_str()),
        })),
        Err(e) => Ok(json!({
            "ok": false,
            "latency_ms": start.elapsed().as_millis(),
            "error": e,
        })),
    }
}
