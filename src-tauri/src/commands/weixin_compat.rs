use serde_json::{json, Value};

// Web 与桌面共用已审计版本线；不把 npm 可变标签当成兼容性保证。
fn policy() -> Value {
    serde_json::from_str(include_str!("../../../src/lib/weixin-compat-policy.json"))
        .expect("微信兼容性策略 JSON")
}

fn version_parts(value: &str) -> Option<[u32; 3]> {
    let value = value.trim().trim_start_matches('v').split("-zh.").next()?;
    let (core, suffix) = value.split_once('-').unwrap_or((value, ""));
    if !suffix.is_empty() && !suffix.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let parts: Vec<u32> = core
        .split('.')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    parts.try_into().ok()
}

fn host_fits(host: [u32; 3], line: &Value) -> bool {
    host >= version_parts(line["hostMin"].as_str().unwrap()).unwrap()
        && line["hostBefore"]
            .as_str()
            .is_none_or(|v| host < version_parts(v).unwrap())
}

pub fn compatibility(host_version: &str, plugin_version: &str) -> Value {
    if let (Some(host), Some(plugin)) = (version_parts(host_version), version_parts(plugin_version))
    {
        for line in policy()["lines"].as_array().unwrap() {
            if plugin >= version_parts(line["pluginMin"].as_str().unwrap()).unwrap()
                && plugin <= version_parts(line["pluginMax"].as_str().unwrap()).unwrap()
            {
                let compatible = host_fits(host, line);
                let range = format!(
                    ">={}{}",
                    line["hostMin"].as_str().unwrap(),
                    line["hostBefore"]
                        .as_str()
                        .map(|v| format!(" <{v}"))
                        .unwrap_or_default()
                );
                return json!({ "compatible": compatible, "compatError": if compatible { String::new() } else {
                    format!("微信插件 {plugin_version} 要求 OpenClaw {range}，当前为 {host_version}。")
                }});
            }
        }
    }
    json!({ "compatible": null, "compatError": "OpenClaw 或微信插件版本尚未核实，请先检查版本；不会自动更新。" })
}

pub fn resolve_version(host_version: &str, requested: Option<&str>) -> Result<String, String> {
    let host = version_parts(host_version)
        .ok_or("未识别 OpenClaw 稳定版本，请先安装或检查内核版本，再安装微信插件。")?;
    let policy = policy();
    let target = requested
        .or_else(|| {
            policy["lines"]
                .as_array()
                .unwrap()
                .iter()
                .rev()
                .find(|line| host_fits(host, line))
                .and_then(|line| line["target"].as_str())
        })
        .unwrap_or_default();
    let check = compatibility(host_version, target);
    if check["compatible"].as_bool() != Some(true) {
        return Err(check["compatError"].as_str().unwrap().into());
    }
    Ok(target.into())
}

pub fn status(host_version: &str, installed_version: Option<&str>) -> Value {
    let target = resolve_version(host_version, None);
    let mut result = compatibility(host_version, installed_version.unwrap_or_default());
    let known = installed_version.is_none() || !result["compatible"].is_null();
    result["hostVersion"] = json!(host_version);
    result["recommendedVersion"] = json!(target.as_ref().ok());
    result["installAllowed"] = json!(target.is_ok() && known);
    result["installError"] = json!(target.as_ref().err().cloned().unwrap_or_else(|| {
        if known {
            String::new()
        } else {
            result["compatError"].as_str().unwrap().into()
        }
    }));
    result["updateAvailable"] = json!(
        known
            && installed_version.is_some_and(|v| {
                target
                    .as_ref()
                    .is_ok_and(|t| version_parts(v) < version_parts(t))
            })
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_compatibility_cases() {
        let cases: Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/weixin-compat.json"))
                .unwrap();
        for case in cases.as_array().unwrap() {
            let actual = status(case["host"].as_str().unwrap(), case["plugin"].as_str());
            for key in [
                "compatible",
                "recommendedVersion",
                "installAllowed",
                "updateAvailable",
            ] {
                assert_eq!(actual[key], case[key], "case {case}, field {key}");
            }
        }
        assert!(resolve_version("2026.3.21", Some("2.4.8")).is_err());
        assert!(resolve_version("2026.9.4", Some("1.0.3")).is_err());
        assert!(resolve_version("2026.9.4", Some("latest")).is_err());
    }
}
