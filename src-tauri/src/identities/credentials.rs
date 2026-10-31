// [INPUT]: 原生默认凭据快照、用户 MCP/插件保留策略与 base64/JSON。
// [OUTPUT]: 仅身份字段变换的钥匙串写入协议；保留当前共享 MCP/插件秘密；有对应目标的 Desktop 条目原地替换以保留本机既有授权。
// [POS]: 保险库和 macOS Security 桥之间的秘密边界，不进入 GUI 或日志。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
use super::Policy;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::Value;
fn object(item: &Value) -> Result<Value, String> {
    let bytes = STANDARD
        .decode(item["data"].as_str().ok_or("身份凭据无法处理。")?)
        .map_err(|_| "身份凭据无法处理。")?;
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| "身份凭据格式不受支持，未修改凭据。")?;
    if !value.is_object() {
        return Err("身份凭据格式不受支持，未修改凭据。".into());
    }
    Ok(value)
}
fn encoded(value: &Value) -> String {
    STANDARD.encode(serde_json::to_vec(value).unwrap())
}
pub fn clear(items: &[Value], policy: &Policy) -> Result<Vec<Value>, String> {
    items
        .iter()
        .map(|item| {
            let mut result = item.clone();
            if item["service"] == "Claude Code-credentials" {
                let mut current = object(item)?;
                for key in crate::cleaner::identity::credential_keys(policy) {
                    current.as_object_mut().unwrap().remove(&key);
                }
                if !current.as_object().unwrap().is_empty() {
                    result["remaining"] = Value::String(encoded(&current));
                }
            }
            Ok(result)
        })
        .collect()
}
pub fn clear_for_restore(items: &[Value], target: &[Value], policy: &Policy) -> Result<Vec<Value>, String> {
    let mut cleared = clear(items, policy)?;
    for item in &mut cleared {
        if item["service"] == "Claude Safe Storage"
            && item["account"].is_string()
            && target.iter().any(|next| next["service"] == item["service"] && next["account"] == item["account"])
        {
            // Keep the verified live item until restore performs its conditional data update.
            // Deleting/recreating it discards the user's existing application ACL. No ACL is
            // imported from the archive, and empty identities still use ordinary deletion.
            item["remaining"] = item["data"].clone();
        }
    }
    Ok(cleared)
}
pub fn restore(items: &[Value], live: &[Value], policy: &Policy) -> Result<Vec<Value>, String> {
    items
        .iter()
        .map(|item| {
            let mut result = item.clone();
            let current = live
                .iter()
                .find(|row| row["service"] == item["service"] && row["account"] == item["account"]);
            if let Some(row) = current {
                result["expected"] = row["data"].clone();
            }
            if item["service"] == "Claude Code-credentials" {
                let saved = object(item)?;
                let mut value = if let Some(current) = current {
                    object(current)?
                } else {
                    serde_json::json!({})
                };
                for key in crate::cleaner::identity::credential_keys(policy) {
                    value.as_object_mut().unwrap().remove(&key);
                    if let Some(v) = saved.get(&key) {
                        value.as_object_mut().unwrap().insert(key, v.clone());
                    }
                }
                result["data"] = Value::String(encoded(&value));
            }
            Ok(result)
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_transition_preserves_only_matching_live_item_until_restore() {
        let row = |account: &str, secret: &str| serde_json::json!({
            "service":"Claude Safe Storage", "account":account, "data":STANDARD.encode(secret), "reference":"live-ref"
        });
        let a = row("Claude", "A-key");
        let b = row("Claude", "B-key");
        let p = Policy::default();
        let plan = clear_for_restore(&[a.clone()], &[b.clone()], &p).unwrap();
        assert_eq!(plan[0]["remaining"], a["data"]);
        assert_eq!(plan[0]["reference"], a["reference"]);
        let restore = restore(&[b.clone()], &[a.clone()], &p).unwrap();
        assert_eq!(restore[0]["data"], b["data"]);
        assert_eq!(restore[0]["expected"], a["data"]);
        for target in [vec![], vec![row("other", "B-key")]] {
            assert!(clear_for_restore(&[a.clone()], &target, &p).unwrap()[0].get("remaining").is_none());
        }
        assert!(clear(&[a], &p).unwrap()[0].get("remaining").is_none());
    }

    #[test]
    fn desktop_transition_does_not_change_cli_clearing() {
        let code = serde_json::json!({"service":"Claude Code", "account":"Claude", "data":STANDARD.encode("api-key")});
        let p = Policy::default();
        assert_eq!(clear_for_restore(&[code.clone()], &[code.clone()], &p).unwrap(), clear(&[code], &p).unwrap());
    }
    #[test]
    fn changing_identity_preserves_current_mcp_and_plugin_secrets() {
        let row = |oauth: &str, mcp: &str| serde_json::json!({"service":"Claude Code-credentials","account":"fixture","data":encoded(&serde_json::json!({"claudeAiOauth":{"accessToken":oauth},"mcpOAuth":mcp,"pluginSecrets":"keep-plugin"}))});
        let current = row("current", "latest");
        let target = row("target", "old");
        let policy = Policy::default();
        let cleared = clear(&[current.clone()], &policy).unwrap();
        let remaining: Value = serde_json::from_slice(
            &STANDARD
                .decode(cleared[0]["remaining"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap();
        assert!(remaining.get("claudeAiOauth").is_none());
        assert_eq!(remaining["mcpOAuth"], "latest");
        let next = restore(&[target], &[current], &policy).unwrap();
        let value = object(&next[0]).unwrap();
        assert_eq!(value["claudeAiOauth"]["accessToken"], "target");
        assert_eq!(value["mcpOAuth"], "latest");
        assert_eq!(value["pluginSecrets"], "keep-plugin");
    }
}
