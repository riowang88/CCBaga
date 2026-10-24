// [INPUT]: 当前独占浏览器资料、已验证的 DNS 模式与共用服务商目录。
// [OUTPUT]: 仅当前 user-data-dir 的严格 DoH；首次接管保存原有两项，切回沿用时恢复。
// [POS]: 不改系统/代理配置；仅调用方确认 B 已退出后执行；安全策略可能覆盖 Chromium 偏好。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{fs, path::Path};
#[derive(Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Mode { #[default] Inherit, Secure }
#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Provider { #[default] Cloudflare, Google, Quad9, Custom }
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config { pub mode: Mode, pub provider: Provider, pub custom_url: String }
#[derive(Deserialize)]
struct Preset { id:String, url:String }
impl Config {
    pub fn template(&self) -> Result<Option<String>,String> {
        if self.mode==Mode::Inherit {return Ok(None);}
        let id=match self.provider {Provider::Cloudflare=>"cloudflare",Provider::Google=>"google",Provider::Quad9=>"quad9",Provider::Custom=>{
            validate_url(&self.custom_url)?;return Ok(Some(self.custom_url.clone()));
        }};
        let providers:Vec<Preset>=serde_json::from_str(include_str!("../../../src/browser-doh-providers.json")).map_err(|_|"DoH 服务商目录无效。")?;
        let endpoint=providers.into_iter().find(|p|p.id==id).ok_or("DoH 服务商目录无效。")?.url;
        validate_url(&endpoint)?;Ok(Some(endpoint))
    }
}
pub fn validate_url(value:&str) -> Result<(),String> {
    let invalid=||"填写单个有效的 HTTPS DoH 地址；不支持用户名、密码、片段或空白，可使用末尾 {?dns} / {&dns} 模板。".to_string();
    if value.is_empty() || value.len()>2048 || value.chars().any(|c|c.is_whitespace()||c.is_control()||c=='\\') {return Err(invalid());}
    let (base,get)=if let Some(v)=value.strip_suffix("{?dns}"){(v,1)}else if let Some(v)=value.strip_suffix("{&dns}"){(v,2)}else{(value,0)};
    if base.contains(['{','}']) {return Err(invalid());}
    let url=reqwest::Url::parse(base).map_err(|_|invalid())?;
    if url.scheme()!="https" || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() || (get==1&&url.query().is_some()) || (get==2&&url.query().is_none()) {return Err(invalid());}
    Ok(())
}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Restore { version:u8, had_dns:bool, original:Map<String,Value> }
fn read_object(path:&Path) -> Result<Map<String,Value>,String> {
    match fs::read(path) {
        Ok(bytes)=>serde_json::from_slice::<Value>(&bytes).ok().and_then(|v|v.as_object().cloned()).ok_or_else(||"浏览器 DNS 设置文件损坏，未覆盖原文件。".into()),
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(Map::new()),
        Err(_)=>Err("无法读取浏览器 DNS 设置文件。".into()),
    }
}
pub(super) fn apply(profile:&Path, config:&Config) -> Result<(),String> {
    let template=config.template()?;
    let marker=profile.join("CCBaga-DNS.json");super::profile::check_path(profile,&marker)?;
    let restore=match fs::read(&marker) {
        Ok(bytes)=>{
            let r:Restore=serde_json::from_slice(&bytes).map_err(|_|"DNS 原设置记录损坏，未覆盖现有解析设置。")?;
            if r.version!=1 || r.original.keys().any(|k|k!="mode"&&k!="templates") {return Err("DNS 原设置记录损坏，未覆盖现有解析设置。".into());}Some(r)
        },
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>None,
        Err(_)=>return Err("无法读取 DNS 原设置记录。".into()),
    };
    // 默认和旧配置不接管 DNS：不重写 Local State，也不覆盖已有安全 DNS。
    if template.is_none()&&restore.is_none(){return Ok(());}
    let path=profile.join("Local State");super::profile::check_path(profile,&path)?;
    let mut state=read_object(&path)?;
    let had_dns=state.contains_key("dns_over_https");
    let mut dns=match state.get("dns_over_https") {
        None=>Map::new(),Some(Value::Object(dns))=>dns.clone(),
        _=>return Err("浏览器 DNS 设置字段无效，未覆盖原文件。".into()),
    };
    if let Some(template)=template {
        if restore.is_none() {
            let original=dns.iter().filter(|(k,_)|k.as_str()=="mode"||k.as_str()=="templates").map(|(k,v)|(k.clone(),v.clone())).collect();
            // 先记录原值，再改 Local State；失败后可重试或恢复，不宣称跨文件事务。
            super::profile::write_json(&marker,&Restore{version:1,had_dns,original})?;
        }
        dns.insert("mode".into(),Value::String("secure".into()));dns.insert("templates".into(),Value::String(template));
        state.insert("dns_over_https".into(),Value::Object(dns));
        super::profile::write_json(&path,&state)?;
    } else if let Some(restore)=restore {
        for key in ["mode","templates"] {match restore.original.get(key){Some(v)=>{dns.insert(key.into(),v.clone());},None=>{dns.remove(key);}}}
        if !restore.had_dns&&dns.is_empty(){state.remove("dns_over_https");}else{state.insert("dns_over_https".into(),Value::Object(dns));}
        super::profile::write_json(&path,&state)?;
        fs::remove_file(&marker).map_err(|_|"原 DNS 设置已恢复，但恢复记录未清除；请重试。")?;
    }
    Ok(())
}
