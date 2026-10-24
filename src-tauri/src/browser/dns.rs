// [INPUT]: 会话独立随机 token、B 触发的 Net.Coffee DNS 记录、配置/系统代理。
// [OUTPUT]: 有界解析器 IP/归属地与真实失败；每轮最多三次取数、一次在途、两分钟过期。
// [POS]: A 只读固定 HTTPS 结果服务，不触发 DNS 测试，不伪造来源/携带 Cookie，不回退直连。
// [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
use serde::Serialize;
use serde_json::Value;
use std::{collections::HashSet, time::{Duration, Instant}};
pub(super) const API: &str = "https://ip.net.coffee";
const MAX_RESOLVERS: usize = 16;
const MAX_BODY: usize = 32768;
pub(super) struct Run {
    pub id: String,
    proxy: String,
    started: Instant,
    polls: u8,
    pub in_flight: bool,
}
impl Run {
    pub fn new(id: String, proxy: String) -> Self {
        Self {id,proxy,started:Instant::now(),polls:0,in_flight:false}
    }
    pub fn can_replace(&self) -> bool {
        !self.in_flight && self.started.elapsed() >= Duration::from_secs(5)
    }
    pub fn begin_poll(&mut self, id: &str) -> Result<String,String> {
        if id != self.id || self.started.elapsed() > Duration::from_secs(120) {
            return Err("DNS 检测轮次已失效，请重新检测。".into());
        }
        if self.in_flight || self.polls >= 3 {
            return Err("DNS 检测正在进行或已达本轮读取上限，请稍后重新检测。".into());
        }
        self.polls += 1;
        self.in_flight = true;
        Ok(self.proxy.clone())
    }
}
#[derive(Clone, Serialize, Debug)]
pub(super) struct Resolver {
    pub ip: String,
    pub country_code: String,
    pub country: String,
    pub city: String,
    pub isp: String,
    pub geo_error: Option<String>,
}
impl Resolver {
    fn unknown(ip: String, error: String) -> Self {
        Self {ip,country_code:String::new(),country:String::new(),city:String::new(),isp:String::new(),geo_error:Some(error)}
    }
}
#[derive(Serialize, Debug)]
pub(super) struct Observation {
    pub id: String,
    pub resolvers: Vec<Resolver>,
    pub observed_at: u64,
}
pub(super) fn parse_ips(data: &Value, id: &str) -> Result<Vec<String>, String> {
    if data["token"].as_str() != Some(id) {return Err("DNS 检测轮次不匹配。".into());}
    let values = data["dns_servers"].as_array().filter(|a| a.len() <= MAX_RESOLVERS)
        .ok_or("DNS 服务返回无效的解析器列表。")?;
    let mut seen=HashSet::new();
    let mut ips=Vec::new();
    for value in values {
        let ip=value.as_str().and_then(|s| s.parse::<std::net::IpAddr>().ok())
            .ok_or("DNS 服务返回无效的解析器 IP。")?.to_string();
        if seen.insert(ip.clone()) {ips.push(ip);}
    }
    Ok(ips)
}
fn parse_geo(ip: String, value: &Value) -> Result<Resolver,String> {
    let field=|key:&str| -> Result<String,String> {
        match value.get(key) {
            None|Some(Value::Null)=>Ok(String::new()),
            Some(Value::String(s)) if s.len()<300=>Ok(s.clone()),
            _=>Err("DNS 归属地字段无效。".into()),
        }
    };
    let code=field("country_code")?;
    if code.len()!=2 || !code.bytes().all(|c|c.is_ascii_alphabetic()) {
        return Err("DNS 归属地未返回有效国家代码。".into());
    }
    Ok(Resolver {ip,country_code:code.to_ascii_uppercase(),country:field("country")?,city:field("city")?,isp:field("isp")?,geo_error:None})
}
async fn json(client:&reqwest::Client, url:&str) -> Result<Value,String> {
    let mut r=client.get(url).send().await.map_err(|e|if e.is_timeout(){"DNS 服务请求超时。"}else{"DNS 服务连接失败。"})?;
    if !r.status().is_success() {return Err(format!("DNS 服务返回 HTTP {}",r.status().as_u16()));}
    let mut bytes=Vec::new();
    while let Some(chunk)=r.chunk().await.map_err(|_|"DNS 服务响应读取失败。")? {
        if bytes.len()+chunk.len()>MAX_BODY {return Err("DNS 服务响应过大。".into());}
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_|"DNS 服务返回无效 JSON。".into())
}
pub(super) async fn collect(id:&str, proxy:&str) -> Result<Observation,String> {
    collect_from(id,proxy,API).await
}
async fn collect_from(id:&str, proxy:&str, base:&str) -> Result<Observation,String> {
    // Origin/Referer/Cookie 均不设置；正常原生请求由服务端决定是否接受。
    let mut builder=reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(8));
    if !proxy.is_empty() {builder=builder.proxy(reqwest::Proxy::all(proxy).map_err(|_|"DNS 查询代理地址无效。")?);}
    let client=builder.build().map_err(|_|"无法建立 DNS 查询。")?;
    let operation=async {
        let value=json(&client,&format!("{base}/api/dns/result/{id}")).await?;
        let ips=parse_ips(&value,id)?;
        let mut tasks=tokio::task::JoinSet::new();
        for (index,ip) in ips.into_iter().enumerate() {
            let client=client.clone();let base=base.to_string();
            tasks.spawn(async move {
                let geo=match json(&client,&format!("{base}/api/geoip/{ip}")).await {
                    Ok(value)=>parse_geo(ip.clone(),&value),Err(e)=>Err(e),
                };
                (index,geo.unwrap_or_else(|e|Resolver::unknown(ip,e)))
            });
        }
        let mut ordered=Vec::new();
        while let Some(result)=tasks.join_next().await {
            ordered.push(result.map_err(|_|"DNS 归属地查询中断。")?);
        }
        ordered.sort_by_key(|(index,_)|*index);
        Ok(Observation {id:id.to_string(),resolvers:ordered.into_iter().map(|(_,r)|r).collect(),observed_at:std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64})
    };
    tokio::time::timeout(Duration::from_secs(20),operation).await.map_err(|_|"DNS 服务请求超时。".to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn accepts_observed_provider_shape_and_deduplicates_ipv4_ipv6() {
        let data=json!({"token":"round-a","dns_servers":["1.1.1.1","2606:4700:4700::1111","1.1.1.1"]});
        assert_eq!(parse_ips(&data,"round-a").unwrap(),vec!["1.1.1.1","2606:4700:4700::1111"]);
        assert!(parse_ips(&json!({"token":"round-a","dns_servers":[]}),"round-a").unwrap().is_empty());
    }
    #[test]
    fn rejects_other_rounds_bad_ips_and_missing_results() {
        for data in [json!({"token":"other","dns_servers":["1.1.1.1"]}),json!({"token":"round-a","dns_servers":["not-an-ip"]}),json!({"token":"round-a"}),json!({"token":"round-a","dns_servers":["1.1.1.1",42]})] {
            assert!(parse_ips(&data,"round-a").is_err());
        }
    }
    #[test]
    fn rounds_expire_and_bound_concurrent_or_repeated_reads() {
        let mut run=Run::new("a".into(),"http://127.0.0.1:7890".into());
        assert!(!run.can_replace());
        assert!(run.begin_poll("other").is_err());
        assert_eq!(run.begin_poll("a").unwrap(),"http://127.0.0.1:7890");
        assert!(run.begin_poll("a").is_err());
        run.in_flight=false;run.begin_poll("a").unwrap();
        run.in_flight=false;run.begin_poll("a").unwrap();
        run.in_flight=false;assert!(run.begin_poll("a").is_err());
        run.started=Instant::now()-Duration::from_secs(121);
        assert!(run.can_replace());assert!(run.begin_poll("a").is_err());
    }
    fn mock_server(count:usize, reply:fn(&str)->(u16,String,Vec<(&'static str,&'static str)>)) -> (String,std::thread::JoinHandle<Vec<String>>) {
        use std::io::{Read,Write};
        let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base=format!("http://{}",listener.local_addr().unwrap());
        let handle=std::thread::spawn(move||{
            let mut requests=Vec::new();let deadline=Instant::now()+Duration::from_secs(10);
            while requests.len()<count && Instant::now()<deadline {
                let (mut stream,_)=match listener.accept(){Ok(s)=>s,Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>{std::thread::sleep(Duration::from_millis(5));continue;},Err(e)=>panic!("{e}")};
                stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut raw=Vec::new();while !raw.ends_with(b"\r\n\r\n"){let mut b=[0u8];stream.read_exact(&mut b).unwrap();raw.push(b[0]);}
                let request=String::from_utf8(raw).unwrap();
                let path=request.split_whitespace().nth(1).unwrap();
                let (status,body,headers)=reply(path);
                let extra=headers.into_iter().map(|(k,v)|format!("{k}: {v}\r\n")).collect::<String>();
                write!(stream,"HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nContent-Type: application/json\r\n{extra}Connection: close\r\n\r\n{body}",body.len()).unwrap();
                requests.push(request);
            }
            assert_eq!(requests.len(),count);requests
        });
        (base,handle)
    }
    #[test]
    fn native_reader_uses_only_result_and_geo_endpoints_and_keeps_partial_geo_failures() {
        let (base,server)=mock_server(3,|path| match path {
            "/api/dns/result/round-a"=>(200,json!({"token":"round-a","dns_servers":["1.1.1.1","2001:4860:4860::8888"]}).to_string(),vec![]),
            "/api/geoip/1.1.1.1"=>(200,json!({"country_code":"au","country":"Australia","isp":"Cloudflare"}).to_string(),vec![]),
            "/api/geoip/2001:4860:4860::8888"=>(503,"unavailable".into(),vec![]),
            _=>panic!("Unexpected path {path}"),
        });
        let value=tauri::async_runtime::block_on(collect_from("round-a","",&base)).unwrap();
        assert_eq!(value.resolvers.len(),2);assert_eq!(value.resolvers[0].country_code,"AU");
        assert_eq!(value.resolvers[1].ip,"2001:4860:4860::8888");assert!(value.resolvers[1].geo_error.as_ref().unwrap().contains("503"));
        for request in server.join().unwrap(){let r=request.to_ascii_lowercase();for forbidden in ["origin:","referer:","cookie:","authorization:"]{assert!(!r.contains(forbidden));}assert!(!r.contains("pixel.gif"));}
    }
    #[test]
    fn service_rejection_redirect_and_oversized_body_are_errors_not_empty_success() {
        for status in [403,302,200] {
            let (base,server)=match status {
                403=>mock_server(1,|_|(403,"Forbidden".into(),vec![])),
                302=>mock_server(1,|_|(302,String::new(),vec![("Location","http://127.0.0.1:1/forbidden")])),
                _=>mock_server(1,|_|(200," ".repeat(MAX_BODY+1),vec![])),
            };
            assert!(tauri::async_runtime::block_on(collect_from("round-a","",&base)).is_err());server.join().unwrap();
        }
        assert!(tauri::async_runtime::block_on(collect_from("round-a","http://[invalid",API)).is_err());
    }

}
