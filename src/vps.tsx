/**
 * [INPUT]: 原生只读检测命令：browser_read/browser_check（本机出口与环境一致性）、network_diagnose_domains（四域名探针）、browser_launch（让 B 重新实测与 DNS）、browser_open_report（固定报告页）；browser-report 事件与 YAML 文案。
 * [OUTPUT]: 网络环境检测页：本机出口 IP/所在地/运营商、深色 Claude 可用性四域名卡（含延迟）、环境一致性网格、DNS 最近一轮结果与「在 B 中检测」、WebRTC/UDP 如实「未检测」、「在 B 中打开完整报告」三级链接。
 * [POS]: 纯检测页；不修改系统、浏览器或代理设置，不自算 IP 信任评分/代理标记/住宅属性；DNS 由 B 触发，A 只读同会话最近一轮结果。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import {useEffect,useRef,useState} from 'react';
import {Channel,invoke} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import {useI18n} from './i18n';
import type {BrowserState} from './browser-model';
import './network.css';

export type DomainDiagnosis = {
    domain: string;
    ok: boolean;
    status_code: number | null;
    latency_ms: number | null;
    ip: string | null;
    message: string;
};

const claudeDomains=['claude.ai','api.anthropic.com','platform.claude.com','downloads.claude.ai'] as const;
export function VpsPanel() {
    const {t,locale}=useI18n();
    const time=(value:number)=>new Date(value).toLocaleTimeString(locale==='zh'?'zh-CN':'en-US');
    const [state,setState]=useState<BrowserState|null>(null);
    const [stateError,setStateError]=useState('');
    const [exitChecking,setExitChecking]=useState(false), [exitError,setExitError]=useState('');
    const [diagnosing,setDiagnosing]=useState(false);
    const [diagnostics,setDiagnostics]=useState<DomainDiagnosis[]|null>(null);
    const [diagnosticsError,setDiagnosticsError]=useState('');
    const [action,setAction]=useState(''), [error,setError]=useState('');
    const live=useRef(true);
    const results=claudeDomains.map(domain=>diagnostics?.find(item=>item.domain===domain));
    const passed=results.filter(item=>item?.ok).length;
    const phase=diagnosing?'checking':diagnosticsError?'failed':diagnostics?(passed===claudeDomains.length?'passed':'failed'):'idle';
    const statusText=phase==='checking'?'检测中':phase==='passed'?'检测成功':phase==='failed'?'检测失败':'待检测';
    const checking=diagnosing||exitChecking;
    const started=diagnostics!==null||!!diagnosticsError||!!state?.machine_exit||!!exitError;

    async function read() {
        try { const next=await invoke<BrowserState>('browser_read'); if(live.current){setState(next);setStateError('');} }
        catch(e) { if(live.current)setStateError(String(e)); }
    }
    useEffect(()=>{
        live.current=true;void read();
        const event=listen('browser-report',()=>void read());
        const focus=()=>void read();window.addEventListener('focus',focus);
        return ()=>{live.current=false;window.removeEventListener('focus',focus);void event.then(stop=>stop());};
    },[]);

    async function runExit() {
        setExitChecking(true);setExitError('');
        try { const next=await invoke<BrowserState>('browser_check'); if(live.current)setState(next); }
        catch(e) { if(live.current)setExitError(String(e)); await read(); }
        finally { if(live.current)setExitChecking(false); }
    }

    async function runDiagnostics() {
        setDiagnosing(true);
        setDiagnostics(null);
        setDiagnosticsError('');
        try {
            const results = await invoke<DomainDiagnosis[]>('network_diagnose_domains');
            if(!Array.isArray(results))throw new Error('检测结果无效，请重试。');
            setDiagnostics(results);
            if(claudeDomains.some(domain=>!results.some(item=>item.domain===domain)))setDiagnosticsError('部分检测未返回结果，请重试。');
        } catch (e) {
            setDiagnosticsError(String(e));
        } finally {
            setDiagnosing(false);
        }
    }

    function runAll() {
        if(checking)return;
        setError('');
        void runExit();void runDiagnostics();
    }

    async function inBrowser(command:'browser_launch'|'browser_open_report') {
        if(action)return;
        setAction(command);setError('');
        try {
            if(command==='browser_launch'){const next=await invoke<BrowserState>(command,{onProgress:new Channel()});if(live.current)setState(next);}
            else await invoke(command);
        } catch(e) { if(live.current)setError(String(e)); }
        finally { if(live.current)setAction(''); }
    }

    const exit=state?.machine_exit;
    const browserExit=state?.report?.exit;
    const dns=state?.running?state.dns_last:null;
    const resolvers=dns?.observation?.resolvers;
    const dnsState=!state?.running?'stopped':!dns?'idle':dns.error?'failed':resolvers?.length?'observed':'empty';
    const dnsCountries=[...new Set((resolvers||[]).map(row=>row.country_code).filter(Boolean))];

    return <section className="rb-page network-page" data-page="network">
        <header className="network-head">
            <div>
                <h1 className="rb-h1">{t('网络环境')}</h1>
                <p className="rb-sub">{t('只做检测，不改设置。')}</p>
            </div>
            <button type="button" className="rb-link" disabled={!state?.running||!!action} title={state?.running?undefined:t('请先打开 B 浏览器，再查看完整报告。')} onClick={()=>void inBrowser('browser_open_report')}>{t('在 B 中打开完整报告')} →</button>
            <button type="button" className="rb-btn primary" disabled={checking} aria-busy={checking} onClick={runAll}>{checking?t('检测中'):started?t('全部重新检测'):t('开始检测')}</button>
        </header>
        {error&&<p role="alert" className="rb-err network-feedback">{t(error)}</p>}
        {stateError&&<p role="alert" className="rb-err network-feedback">{t(stateError)}</p>}

        <section className="network-exit" aria-label={t('本机出口')} data-status={exitChecking?'checking':exitError?'failed':exit?'observed':'idle'}>
            <div className="rb-card"><p className="rb-label">{t('本机出口 IP')}</p><p className="network-big rb-mono">{exitChecking?t('检测中'):exit?.ip||'—'}</p></div>
            <div className="rb-card"><p className="rb-label">{t('所在地')}</p><p className="network-big">{exit?[exit.country,exit.city].filter(Boolean).join(' · '):'—'}</p></div>
            <div className="rb-card"><p className="rb-label">{t('运营商')}</p><p className="network-big network-isp">{exit?.isp||'—'}</p></div>
        </section>
        <p className="rb-muted network-note">{exitError?<span className="rb-err" role="alert">{t(exitError)}</span>:exit?t('由 A 按系统代理检测；出口数据库给出的时区：{v0}',{v0:exit.timezone||'—'}):t('尚未检测本机出口。')}{browserExit&&<> · {t('B 浏览器出口：{v0}',{v0:`${browserExit.ip} · ${browserExit.country}`})}</>}</p>

        <div className="network-grid">
            <section className="rb-card dark domain-diagnostics" aria-labelledby="diagnostics-title" data-status={phase} aria-busy={diagnosing}>
                <h2 id="diagnostics-title" className="rb-label">{t('Claude 可用性')}</h2>
                <div className={`domain-outcome ${phase}`} role="status" aria-live="polite">
                    <strong>{t(statusText)}</strong>
                    {diagnostics&&!diagnosticsError&&<span>{t('已通过 {v0} / {v1} 项',{v0:passed,v1:claudeDomains.length})}</span>}
                    {diagnosticsError&&<p>{t(diagnosticsError)}</p>}
                </div>
                <ul className="domain-diagnostics-list">{claudeDomains.map((domain,index)=>{
                    const item=results[index];
                    const status=diagnosing?'checking':item?item.ok?'passed':'failed':phase==='failed'?'failed':'idle';
                    return <li key={domain} className={`rb-row domain-diagnostics-item ${status}`}>
                        <div className="domain-item-info"><span className="domain-name rb-mono">{domain}</span>{item&&<span className="domain-detail">{item.status_code?`HTTP ${item.status_code} · `:''}{t(item.message)}</span>}{item?.ip&&<span className="domain-ip rb-mono" title={t('连接对端，不代表出口 IP')}>{item.ip}</span>}</div>
                        {item?.latency_ms!=null&&<span className="domain-latency rb-mono">{item.latency_ms} ms</span>}
                        <span className={`domain-badge ${status}`}>{t(status==='checking'?'检测中':item?item.ok?'检测成功':'检测失败':phase==='failed'?'未完成':'待检测')}</span>
                    </li>;
                })}</ul>
                <p className="network-dark-note">{t('只读连通性探测：不携带认证、不跟随重定向；连接对端不代表出口 IP。')}</p>
            </section>

            <section className="rb-card dark network-consistency" aria-labelledby="consistency-title">
                <h2 id="consistency-title" className="rb-label">{t('环境一致性 · {v0}',{v0:t(state?.region?.label||'自定义环境')})}</h2>
                <p className="network-dark-note">{state?.report_received_at?t('已连接 · 回报 {v0}',{v0:time(state.report_received_at)}):state?.running?t('等待浏览器回报'):t('浏览器未运行')}</p>
                <ul className="network-check-list">{(state?.checks||[]).map(check=><li key={check.title} className={`rb-row network-check check-${check.status}`}>
                    <div><strong>{t(check.title)}</strong><p>{t(check.detail)}</p></div>
                    <span className={check.status==='ok'?'rb-ok':check.status==='mismatch'?'rb-err':'rb-warn'} aria-label={t(check.status==='ok'?'已核验':check.status==='mismatch'?'存在冲突':'尚未核验')}>{t(check.status==='ok'?'已核验':check.status==='mismatch'?'存在冲突':'尚未核验')}</span>
                </li>)}</ul>
                {state?.report?.error&&<p className="network-dark-note">{t('浏览器出口检测：')}{t(state.report.error)}</p>}
            </section>
        </div>

        <div className="network-grid">
            <section className="rb-card line network-dns" aria-labelledby="network-dns-title" data-dns={dnsState}>
                <div className="network-card-head"><h2 id="network-dns-title" className="rb-label">{t('DNS 泄露')}</h2>
                    <button type="button" className="rb-link" disabled={!!action||!state?.engine_ready} onClick={()=>void inBrowser('browser_launch')}>{action==='browser_launch'?t('正在打开实测页'):state?.running?t('在 B 中检测'):t('打开 B 并检测')} →</button></div>
                <p role="status">{dnsState==='stopped'?<span className="rb-muted">{t('B 浏览器未运行；DNS 由 B 触发解析，A 只读取最近一轮结果。')}</span>
                    :dnsState==='idle'?<span className="rb-muted">{t('尚未检测 DNS')}</span>
                    :dnsState==='failed'?<span className="rb-err">{t('DNS 检测未完成')} · {t(dns!.error)}</span>
                    :dnsState==='empty'?<span className="rb-warn">{t('未取得解析器结果，无法判断 DNS 状态；空结果不代表安全或已加密。')}</span>
                    :<span>{t('共观测到 {v0} 个解析器 · {v1}',{v0:resolvers!.length,v1:time(dns!.observation!.observed_at)})}{dnsCountries.length>0&&<span className="rb-muted"> · {dnsCountries.join(' / ')}</span>}</span>}</p>
                {!!resolvers?.length&&<table className="network-dns-table"><thead><tr><th>{t('DNS 解析器 IP')}</th><th>{t('国家 / 城市')}</th><th>{t('运营商')}</th></tr></thead><tbody>{resolvers.map(row=><tr key={row.ip}><td className="rb-mono">{row.ip}</td><td>{row.geo_error?<span title={t(row.geo_error)}>{t('归属地未取得')}</span>:[row.country_code,row.country,row.city].filter(Boolean).join(' · ')}</td><td>{row.isp||t('未知')}</td></tr>)}</tbody></table>}
                <p className="rb-muted network-small">{t('解析器地区与出口不同不自动等于泄露；结果不构成安全保证。')}</p>
            </section>
            <section className="rb-card line network-webrtc" aria-labelledby="network-webrtc-title" data-webrtc="unchecked">
                <h2 id="network-webrtc-title" className="rb-label">{t('WebRTC / UDP 泄露')}</h2>
                <p role="status"><span className="rb-warn">{t('未检测')}</span><span className="rb-muted"> · {t('本版本尚未提供 WebRTC/UDP 检测；未检测不代表通过。')}</span></p>
            </section>
        </div>
        <p className="rb-muted network-small network-foot">{t('IP 信任评分、VPN/代理标记与住宅/机房属性不在此显示，也不自行推算；需要时在 B 中打开完整报告查看。')}</p>
    </section>;
}
