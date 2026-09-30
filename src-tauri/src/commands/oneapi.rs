/// OneAPI 余额/充值代理命令
/// 通过用户名 + 密码登录云端 OneAPI 会话，代理查询余额、测试连接等操作
use serde_json::{json, Value};

/// OneAPI / New API 默认计价单位（1 美元 = 500000 额度）
const DEFAULT_QUOTA_PER_UNIT: f64 = 500_000.0;

/// 钱包登录后自动写入 openclaw.json 的 provider 名
const PROVIDER_KEY_NEWAPI: &str = "newapi";

/// 构建 newapi provider 配置。new-api 暴露的是 OpenAI 兼容协议，
/// 所以统一用 openai-completions，baseUrl 指向 <url>/v1。
fn build_newapi_provider(url: &str, api_key: &str, model_ids: &[String]) -> Value {
    let base_url = format!("{}/v1", url.trim_end_matches('/'));
    let models: Vec<Value> = model_ids
        .iter()
        .map(|id| json!({ "id": id, "name": id }))
        .collect();
    json!({
        "baseUrl": base_url,
        "apiKey": api_key,
        "api": "openai-completions",
        "models": models,
    })
}

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

/// 登录 OneAPI，返回 (会话 Cookie, 用户数字 ID, 用户名)
async fn oneapi_login(
    client: &reqwest::Client,
    url: &str,
    username: &str,
    password: &str,
) -> Result<(String, Option<i64>, String), String> {
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
    let user_id = data.get("id").and_then(|v| v.as_i64());
    let resolved_name = data
        .get("username")
        .and_then(|v| v.as_str())
        .unwrap_or(username)
        .to_string();
    Ok((cookie, user_id, resolved_name))
}

/// 校验 OneAPI 业务响应：new-api 业务失败可能仍是 HTTP 200
fn check_oneapi_body(body: &Value, action: &str) -> Result<(), String> {
    if body.get("success").and_then(|v| v.as_bool()) == Some(false) {
        let msg = body
            .get("message")
            .and_then(|v| v.as_str())
            .filter(|m| !m.is_empty())
            .unwrap_or("未知错误");
        return Err(format!("{action}失败: {msg}"));
    }
    Ok(())
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
    let (cookie, user_id, _) = oneapi_login(&client, url, username, password).await?;
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
/// 在账号下挑选可用的 relay token。
/// 优先复用 new-api 注册时自动生成的「<username>的初始令牌」，
/// 其次取最早创建的「启用 + 永不过期 + 无限额度」token。
/// 返回 (token 对象, 是否为本次新建)
async fn ensure_relay_token(
    client: &reqwest::Client,
    url: &str,
    cookie: &str,
    user_id: i64,
    username: &str,
) -> Result<(Value, bool), String> {
    let list_tokens = || async {
        let body: Value = client
            .get(format!("{url}/api/token/?p=1&page_size=100"))
            .header("Cookie", cookie)
            .header("New-Api-User", user_id.to_string())
            .send()
            .await
            .map_err(|e| format!("查询令牌失败: {e}"))?
            .json()
            .await
            .map_err(|e| format!("解析令牌列表失败: {e}"))?;
        check_oneapi_body(&body, "查询令牌")?;
        let items = body
            .pointer("/data/items")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        Ok::<Vec<Value>, String>(items)
    };

    let initial_name = format!("{username}的初始令牌");
    let enabled = |t: &Value| t.get("status").and_then(|v| v.as_i64()) == Some(1);
    let unlimited = |t: &Value| t.get("unlimited_quota").and_then(|v| v.as_bool()) == Some(true);
    let never_expires = |t: &Value| t.get("expired_time").and_then(|v| v.as_i64()) == Some(-1);

    let pick = |items: &[Value]| -> Option<Value> {
        if let Some(t) = items
            .iter()
            .find(|t| enabled(t) && t.get("name").and_then(|v| v.as_str()) == Some(initial_name.as_str()))
        {
            return Some(t.clone());
        }
        items
            .iter()
            .filter(|t| enabled(t) && unlimited(t) && never_expires(t))
            .min_by_key(|t| t.get("id").and_then(|v| v.as_i64()).unwrap_or(i64::MAX))
            .cloned()
    };

    if let Some(token) = pick(&list_tokens().await?) {
        return Ok((token, false));
    }

    // 没有可用 token：建一个 ClawPanel 专用令牌
    let body: Value = client
        .post(format!("{url}/api/token/"))
        .header("Cookie", cookie)
        .header("New-Api-User", user_id.to_string())
        .json(&json!({
            "name": "ClawPanel",
            "expired_time": -1,
            "unlimited_quota": true,
            "remain_quota": 0,
        }))
        .send()
        .await
        .map_err(|e| format!("创建令牌失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("解析创建令牌响应失败: {e}"))?;
    check_oneapi_body(&body, "创建令牌")?;

    // 创建接口不回传 key，必须重新列表读取
    let items = list_tokens().await?;
    let token = pick(&items).ok_or("创建令牌后仍未在列表中找到可用令牌")?;
    Ok((token, true))
}

/// 读取会话令牌列表，返回 items 数组
async fn fetch_token_items(
    client: &reqwest::Client,
    url: &str,
    cookie: &str,
    user_id: i64,
) -> Result<Vec<Value>, String> {
    let body: Value = client
        .get(format!("{url}/api/token/?p=1&page_size=100"))
        .header("Cookie", cookie)
        .header("New-Api-User", user_id.to_string())
        .send()
        .await
        .map_err(|e| format!("查询令牌失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("解析令牌列表失败: {e}"))?;
    check_oneapi_body(&body, "查询令牌")?;
    Ok(body
        .pointer("/data/items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default())
}

/// 从 /v1/models 响应里提取模型 id 列表（兼容对象与字符串两种元素）
fn parse_model_ids(body: &Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(items) = body.get("data").and_then(|v| v.as_array()) {
        for item in items {
            let id = match item {
                Value::String(s) => Some(s.clone()),
                Value::Object(_) => item
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                _ => None,
            };
            if let Some(id) = id.filter(|s| !s.trim().is_empty()) {
                let id = id.trim().to_string();
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
    }
    out
}

/// 用 relay key 拉取可调用模型；失败时回退到会话接口
async fn fetch_model_ids(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    cookie: &str,
    user_id: i64,
) -> Vec<String> {
    if let Ok(resp) = client
        .get(format!("{url}/v1/models"))
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(body) = resp.json::<Value>().await {
                let ids = parse_model_ids(&body);
                if !ids.is_empty() {
                    return ids;
                }
            }
        }
    }

    // 兜底：/api/user/models 返回分组内全部模型（含未配比率的）
    client
        .get(format!("{url}/api/user/models"))
        .header("Cookie", cookie)
        .header("New-Api-User", user_id.to_string())
        .send()
        .await
        .ok()
        .and_then(|r| r.json::<Value>().await.ok())
        .map(|b| parse_model_ids(&b))
        .unwrap_or_default()
}

/// 确保 openclaw.json 里存在名为 `newapi` 的 provider，指向当前 OneAPI 网关。
/// 幂等：内容一致时不写文件、不需要重启网关。
/// 钱包页调用的统一入口：确保 provider 存在，成功后返回可展示的摘要。
/// 幂等：provider 已一致时 changed=false，前端据此跳过重启。
#[tauri::command]
pub async fn oneapi_ensure_provider() -> Result<Value, String> {
    let (url, username, password) =
        read_oneapi_config().ok_or("请先在钱包页面配置 OneAPI 连接信息")?;
    let client = build_client()?;
    let (cookie, user_id, resolved_name) = oneapi_login(&client, &url, &username, &password).await?;
    let user_id = user_id.ok_or("无法获取 OneAPI 用户 ID")?;

    let (token, created_token) =
        ensure_relay_token(&client, &url, &cookie, user_id, &resolved_name).await?;

    let raw_key = token
        .get("key")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .ok_or("令牌列表中缺少 key 字段")?;
    // new-api 存的是 48 位裸值，客户端要 sk- 前缀
    let api_key = if raw_key.starts_with("sk-") {
        raw_key.to_string()
    } else {
        format!("sk-{raw_key}")
    };

    let model_ids = fetch_model_ids(&client, &url, &api_key, &cookie, user_id).await;

    let token_name = token.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let token_mask = if raw_key.len() > 12 {
        format!("{}…{}", &raw_key[..6], &raw_key[raw_key.len() - 4..])
    } else {
        raw_key.to_string()
    };

    let provider = build_newapi_provider(&url, &api_key, &model_ids);
    let (changed, existing_models_kept) =
        super::config::upsert_openclaw_provider(PROVIDER_KEY_NEWAPI, &provider)?;

    Ok(json!({
        "ok": true,
        "changed": changed,
        "provider": PROVIDER_KEY_NEWAPI,
        "token_name": token_name,
        "token_mask": token_mask,
        "created_token": created_token,
        "model_count": model_ids.len(),
        "existing_models_kept": existing_models_kept,
        "url": url,
        "username": resolved_name,
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

/// 获取充值方式与金额选项
#[tauri::command]
pub async fn oneapi_get_topup_info() -> Result<Value, String> {
    let (url, username, password) =
        read_oneapi_config().ok_or("请先在钱包页面配置 OneAPI 连接信息")?;
    let client = build_client()?;
    let (cookie, user_id, _) = oneapi_login(&client, &url, &username, &password).await?;

    let mut req = client
        .get(format!("{url}/api/user/topup/info"))
        .header("Cookie", cookie);
    if let Some(id) = user_id {
        req = req.header("New-Api-User", id.to_string());
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("请求充值信息失败: {e}"))?;
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("解析充值信息失败: {e}"))?;

    let data = body.get("data").cloned().unwrap_or(body);
    Ok(data)
}

/// 请求易支付充值，返回支付链接用于在 ClawPanel 内生成二维码
#[tauri::command]
pub async fn oneapi_request_epay(amount: i64, payment_method: String) -> Result<Value, String> {
    let (url, username, password) =
        read_oneapi_config().ok_or("请先在钱包页面配置 OneAPI 连接信息")?;
    let client = build_client()?;
    let (cookie, user_id, _) = oneapi_login(&client, &url, &username, &password).await?;

    let mut req = client
        .post(format!("{url}/api/user/pay"))
        .header("Cookie", cookie)
        .json(&json!({"amount": amount, "payment_method": payment_method}));
    if let Some(id) = user_id {
        req = req.header("New-Api-User", id.to_string());
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("支付请求失败: {e}"))?;
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("解析支付响应失败: {e}"))?;

    if body.get("message").and_then(|v| v.as_str()) == Some("error") {
        let msg = body
            .get("data")
            .and_then(|v| v.as_str())
            .unwrap_or("支付请求失败");
        return Err(msg.to_string());
    }

    let pay_url = body
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let params = body.get("data").cloned().unwrap_or(json!({}));

    // 用参数构建完整支付 URL（支持 GET 方式，微信/支付宝扫码可直接打开）
    let mut full_url = pay_url.clone();
    if let Some(obj) = params.as_object() {
        let mut parts: Vec<String> = Vec::new();
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                parts.push(format!(
                    "{}={}",
                    urlencoding::encode(k),
                    urlencoding::encode(s)
                ));
            }
        }
        if !parts.is_empty() {
            full_url = format!("{}?{}", pay_url, parts.join("&"));
        }
    }

    Ok(json!({
        "pay_url": full_url,
    }))
}

#[cfg(test)]
mod newapi_provider_tests {
    use super::build_newapi_provider;
    use super::parse_model_ids;
    use serde_json::json;

    /// new-api 存 48 位裸 key，写入 OpenClaw 必须补 sk- 前缀
    #[test]
    fn provider_gets_v1_suffix_and_openai_completions_api() {
        let provider = build_newapi_provider("http://localhost:3000/", "sk-abc", &[]);
        assert_eq!(provider["baseUrl"], "http://localhost:3000/v1");
        assert_eq!(provider["api"], "openai-completions");
        assert_eq!(provider["apiKey"], "sk-abc");
    }

    #[test]
    fn models_are_rendered_as_id_name_pairs() {
        let ids = vec!["deepseek-chat".to_string(), "glm-4".to_string()];
        let provider = build_newapi_provider("http://x", "sk-1", &ids);
        let models = provider["models"].as_array().unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0]["id"], "deepseek-chat");
        assert_eq!(models[0]["name"], "deepseek-chat");
        assert_eq!(models[1]["id"], "glm-4");
    }

    /// /v1/models 返回对象数组，/api/user/models 返回字符串数组
    #[test]
    fn parse_model_ids_handles_both_endpoint_shapes() {
        let objects = json!({ "object": "list", "data": [{ "id": "deepseek-chat" }] });
        assert_eq!(parse_model_ids(&objects), vec!["deepseek-chat"]);

        let strings = json!({ "success": true, "data": ["glm-4", "glm-4v"] });
        assert_eq!(parse_model_ids(&strings), vec!["glm-4", "glm-4v"]);
    }

    #[test]
    fn parse_model_ids_dedupes_and_drops_blank_entries() {
        let body = json!({ "data": ["a", "a", "  ", "", null, { "id": "b" }] });
        assert_eq!(parse_model_ids(&body), vec!["a", "b"]);
    }

    /// 网关 0 channel 时 data 为空数组，必须得到空列表而不是 panic
    #[test]
    fn parse_model_ids_tolerates_empty_and_missing_data() {
        assert!(parse_model_ids(&json!({ "data": [] })).is_empty());
        assert!(parse_model_ids(&json!({ "error": "no channel" })).is_empty());
        assert!(parse_model_ids(&json!({ "data": { "not": "array" } })).is_empty());
    }
}
