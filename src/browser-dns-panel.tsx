/**
 * [INPUT]: B 实测轮次、回环连接状态和 A 返回的真实 DNS 观测。
 * [OUTPUT]: DNS 设置路径说明及解析器 IP/地区/运营商、空结果/失败/部分归属地未知与重新检测；不输出安全保证。
 * [POS]: browser.html 的独立 DNS 结果区，沿用现有 12 项基础环境规则，不把空结果计为通过。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import type {BrowserConfig} from './browser-model';
import {DEFAULT_DNS,dohEndpoint} from './browser-doh';
import {useEffect,useState} from 'react';
import {Button} from 'cromma-ui';
import {useI18n} from './i18n';
import {probeDns,requestDns,triggerDns,waitDns,type DnsObservation} from './browser-dns';
export function BrowserDns({revision,connected,config}:{revision:number;connected:boolean;config?:BrowserConfig}) {
 const {t,locale}=useI18n();
 const dns=config?.dns??DEFAULT_DNS;
 const [retry,setRetry]=useState(0),[result,setResult]=useState<DnsObservation|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState('');
 useEffect(()=>{
  setResult(null);setError('');
  if(!connected||!revision){setBusy(false);return;}
  const controller=new AbortController();setBusy(true);
  void probeDns({signal:controller.signal,request:requestDns,trigger:triggerDns,wait:waitDns}).then(next=>{if(!controller.signal.aborted)setResult(next);}).catch(e=>{if(!controller.signal.aborted)setError(e instanceof Error?e.message:String(e));}).finally(()=>{if(!controller.signal.aborted)setBusy(false);});
  return ()=>controller.abort();
 },[revision,connected,retry]);
 const state=!connected?'disconnected':busy?'checking':error?'failed':result?.resolvers.length?'observed':result?'empty':'idle';
 return <section className="verification-dns" aria-labelledby="verification-dns-title" aria-busy={busy} data-dns={state}>
  <header><h2 id="verification-dns-title">{t('DNS 解析器实测')}</h2><span role="status">{t(!connected?'主应用未连接':busy?'正在检测 DNS…':error?'DNS 检测未完成':result?.resolvers.length?'已取得 DNS 观测':result?'未取得解析器结果':'尚未检测 DNS')}</span><Button variant="ink" disabled={!connected||busy||!revision} loading={busy} onClick={()=>setRetry(n=>n+1)}>{t('重新检测 DNS')}</Button></header>
  <p className="browser-doh-endpoint">{t('DNS 配置目标：{v0}',{v0:t(dns.mode==='secure'?'指定 DoH':'沿用现有解析')})}{dns.mode==='secure'&&` · ${dohEndpoint(dns)}`}</p>
  <p>{t('配置目标不代表已验证使用该解析器；代理代为解析、启动解析和策略覆盖需另行核对。')}</p>
  <p>{t('由当前 B 浏览器触发解析，CCBaga-A 读取 Net.Coffee 的本轮结果。')}</p>
  {error&&<p className="verification-dns-error" role="alert">{t(error)}</p>}
  {result&&result.resolvers.length===0&&<p>{t('未取得解析器结果，无法判断 DNS 状态；空结果不代表安全或已加密。')}</p>}
  {!!result?.resolvers.length&&<><div className="verification-dns-table"><table><thead><tr><th>{t('DNS 解析器 IP')}</th><th>{t('国家 / 城市')}</th><th>{t('运营商')}</th></tr></thead><tbody>{result.resolvers.map(row=><tr key={row.ip}><td>{row.ip}</td><td>{row.geo_error?<span title={t(row.geo_error)}>{t('归属地未取得')}</span>:[row.country_code,row.country,row.city].filter(Boolean).join(' · ')}</td><td>{row.isp||t('未知')}</td></tr>)}</tbody></table></div><p>{t('共观测到 {v0} 个解析器 · {v1}',{v0:result.resolvers.length,v1:new Date(result.observed_at).toLocaleTimeString(locale==='zh'?'zh-CN':'en-US')})}</p></>}
  <p>{t('DNS 观测单独列示，不计入上方 12 项基础环境核验；解析器地区差异不自动等于泄露，结果不构成账号安全保证。')}</p>
  <details className="verification-setting-help verification-dns-help"><summary>{t("DNS 在哪里配置")}</summary><p>{t("本表是实际观测，不能勾选某个解析器让浏览器改用它。在 A → 浏览器环境 → DNS 解析设置选择沿用现有解析或指定 DoH，保存并重开 B 后生效。不要把表里的出口 IP 当作可填写的 DNS 地址。")}</p><h3>{t("代理工具统一处理")}</h3><p>{t("使用 TUN 或本地代理时，在代理工具中检查 DNS 接管、上游解析器、DNS 上游的路由、IPv6 和失败回退。仅开启 TUN 或填写 1.1.1.1 不保证查询经过代理；浏览器自己的加密 DNS 也要纳入同一规则。")}</p><h3>{t("浏览器安全 DNS（DoH）")}</h3><p>{t("在 B 的地址栏输入 chrome://settings/security，查看“使用安全 DNS”。可按引擎提供的选项选择服务商或自定义 HTTPS 地址。自动模式可能回退普通 DNS；指定服务商也不代表请求一定经过代理。")}</p><h3>{t("系统 DNS")}</h3><p>{t("在当前网络连接的 DNS 设置中修改，会影响其他应用。普通 DNS 地址只指定解析器，不提供加密或代理路由；由 TUN 接管时还需检查代理工具的实际规则。")}</p><h3>{t("怎样验证没有意外直连")}</h3><p>{t("先确定预期的解析路径，再让网站域名查询走该路径，避免失败时回退到不期望的系统 DNS。修改后重启 B 并重测，必要时核对代理日志或抓包；单次解析器名单不能证明所有流量都不泄露。")}</p></details>
 </section>;
}
