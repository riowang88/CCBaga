/**
 * [INPUT]: 原生浏览器命令（读取/保存/启动/退出/清理/默认浏览器）、共用 DoH 目录、Cromma Select/Checkbox、共享活动日志与 YAML 文案。
 * [OUTPUT]: 浏览器配置页：B 运行状态与资料绑定标记、打开/退出、设为默认浏览器（系统确认）、指纹种子与「随机换一个」、代理、语言时区跟随/手动、DNS 沿用/指定 DoH、设备指纹模式、保存设置、仅缓存清理与当前身份全部本地资料清除。
 * [POS]: 浏览器配置边界；出口与泄露检测归网络环境页；页面加载只读，资料删除必须由用户选择；全清理使用默认取消且需勾选确认的 AlertDialog，不新增备份，不影响其他身份资料。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import { Button, Select, Checkbox } from 'cromma-ui';
import { useEffect, useRef, useState } from 'react';
import {useI18n} from "./i18n";
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { AppDialog } from './dialog';
import { CleanupActivity, useCleanupActivity } from './cleaner-activity';
import type { BrowserConfig, BrowserState, DefaultBrowser } from './browser-model';
import {DEFAULT_DNS,DOH_PROVIDERS,dohEndpoint,validateDoh,type DnsConfig} from './browser-doh';
import { formatBytes } from './cleaner-model';
import './browser-page.css';
function Segment<T extends string|boolean>({label,value,options,disabled,onChange}:{label:string;value:T;options:{value:T;label:string}[];disabled?:boolean;onChange:(value:T)=>void}) {
    return <div className="rb-seg" role="radiogroup" aria-label={label}>{options.map(option=><button type="button" role="radio" key={String(option.value)} aria-checked={option.value===value} className={option.value===value?'on':undefined} disabled={disabled} onClick={()=>onChange(option.value)}>{option.label}</button>)}</div>;
}
function randomSeed(current:number) {
    const value=new Uint32Array(1);
    do crypto.getRandomValues(value); while(value[0]===0||value[0]===current);
    return value[0];
}
export function BrowserPanel({ onBusy }: {
    onBusy?: (busy: boolean) => void;
}) {
    const {t}=useI18n();
    const [state, setState] = useState<BrowserState | null>(null), [draft, setDraft] = useState<BrowserConfig | null>(null), [busy, setBusy] = useState(''), [error, setError] = useState(''), [confirm, setConfirm] = useState(false);
    const gate = useRef(false), live = useRef(true);
    const [accepted, setAccepted] = useState(false);
    const activity = useCleanupActivity();
    const [preferred, setPreferred] = useState<DefaultBrowser | null>(null), [asking, setAsking] = useState(false), [defaultNote, setDefaultNote] = useState('');
    useEffect(() => { onBusy?.(!!busy); return () => onBusy?.(false); }, [busy, onBusy]);
    async function readDefault() { try {
        const next = await invoke<DefaultBrowser>('browser_default_read');
        if (live.current)
            setPreferred(next);
        return next;
    }
    catch (e) {
        if (live.current)
            setDefaultNote(String(e));
        return null;
    } }
    async function makeDefault() { setAsking(true); setDefaultNote(''); try {
        const next = await invoke<DefaultBrowser>('browser_set_default');
        setPreferred(next);
        setDefaultNote(next.is_default ? "RioBaga-B已是默认浏览器。" : "已向系统请求设为默认浏览器，请检查系统确认。");
        for (let i = 0; i < 20 && live.current; i++) {
            await new Promise(r => setTimeout(r, 1500));
            const fresh = await readDefault();
            if (fresh?.is_default) {
                setDefaultNote("RioBaga-B已设为默认浏览器。");
                break;
            }
        }
    }
    catch (e) {
        setDefaultNote(String(e));
    }
    finally {
        if (live.current)
            setAsking(false);
    } }
    async function read(initial = false) { const next = await invoke<BrowserState>('browser_read'); if (live.current) {
        setState(next);
        if (initial)
            setDraft(next.config);
    } return next; }
    useEffect(() => { live.current = true; void read(true).catch(e => setError(String(e))); void readDefault(); const event = listen('browser-report', () => { if (!gate.current)
        void read().catch(e => setError(String(e))); }); const focus = () => { void readDefault(); if (!gate.current)
        void read().catch(e => setError(String(e))); }; window.addEventListener('focus', focus); return () => { live.current = false; window.removeEventListener('focus', focus); void event.then(f => f()); }; }, []);
    async function act(command: string, args: Record<string, unknown> = {}) { if (gate.current)
        return; gate.current = true; setBusy(command==='browser_clear'&&args.resetProfile?'browser_reset':command); setError(''); const onProgress = activity.start('browser'); try {
        const next = await invoke<BrowserState>(command, { ...args, ...(['browser_launch', 'browser_clear'].includes(command) ? { onProgress } : {}) });
        setState(next);
        if (command === 'browser_save')
            setDraft(next.config);
        activity.finish(true, command === 'browser_clear' ? (args.resetProfile ? "当前身份的浏览器资料已全清理，未保留副本。" : "清理完成，未保留副本。") : "操作完成。");
    }
    catch (e) {
        setError(String(e));
        activity.finish(false, String(e));
        try {
            await read();
        }
        catch {
            setState(null);
        }
    }
    finally {
        gate.current = false;
        setBusy('');
    } }
    const dns=draft?.dns??DEFAULT_DNS;
    const dnsError=validateDoh(dns);
    function editDns(patch:Partial<DnsConfig>) { edit('dns',{...dns,...patch}); }
    const disabled = !!busy || !state || !draft;
    const locked = disabled || !!state?.running;
    const changed = state && draft && JSON.stringify(state.config) !== JSON.stringify(draft);
    const cache = state?.cache.filter(x => !x.login) || [];
    function edit<K extends keyof BrowserConfig>(key: K, value: BrowserConfig[K]) { setDraft(old => old ? { ...old, [key]: value } : old); }
    function closeConfirm() { setConfirm(false); setAccepted(false); }
    return <section className="rb-page browser-page" data-page="browser">
    <header className="browser-head"><div><h1 className="rb-h1">{t("浏览器环境")}</h1><p className="rb-sub">{t("只管 B 浏览器的配置和数据；出口与泄露检测在「网络环境」。")}</p></div><button type="button" className="rb-link" disabled={!!busy} onClick={() => void act('browser_read')}>{t("刷新状态")}</button></header>
    <div className="rb-card dark browser-status" role="group" aria-label={t("B 浏览器状态")}>
      <div className="browser-status-text"><span className={state?.running ? 'rb-ok' : undefined}>{state?.running ? t("B 浏览器运行中") : t("B 浏览器未运行")}</span>{state?.identity_linked&&<span className="browser-status-meta"> · {t("浏览器资料随身份切换")}</span>}{preferred&&<span className="browser-status-meta"> · {t(preferred.is_default?"已是默认浏览器":"不是默认浏览器")}</span>}</div>
      {!preferred?.is_default&&<button type="button" className="rb-link" disabled={!!busy || asking || !preferred?.installed} aria-busy={asking} onClick={() => void makeDefault()}>{t("设为默认浏览器")}</button>}
      <button type="button" className="rb-btn" disabled={disabled || !state?.running} aria-busy={busy === 'browser_quit'} onClick={() => void act('browser_quit')}>{t("正常退出浏览器")}</button>
      <button type="button" className="rb-btn primary" disabled={disabled || !state?.engine_ready || !!changed} aria-busy={busy === 'browser_launch'} onClick={() => void act('browser_launch')}>{state?.running ? t("重新打开实测页") : t("打开浏览器并实测")}</button>
    </div>
    {defaultNote && <p className="rb-muted browser-feedback" role="status">{t(defaultNote)}</p>}
    {error && <p role="alert" className="rb-err browser-feedback">{t(error)}</p>}
    {!!changed && <p className="rb-muted browser-feedback">{t("先保存指纹设置，再启动浏览器。")}</p>}
    <section className="rb-card line browser-settings" aria-labelledby="browser-settings-title">
      <h2 id="browser-settings-title" className="rb-label">{t("指纹与网络")}</h2>
      <div className="rb-row"><label htmlFor="fingerprint-seed">{t("稳定指纹种子")}</label><input id="fingerprint-seed" className="browser-input rb-mono" type="number" min={1} max={4294967295} value={draft?.seed ?? ''} disabled={locked} onChange={e => edit('seed', Number(e.currentTarget.value))}/><button type="button" className="rb-link" disabled={locked} onClick={() => draft && edit('seed', randomSeed(draft.seed))}>{t("随机换一个")}</button></div>
      <div className="rb-row browser-row-wrap"><label htmlFor="browser-proxy">{t("浏览器代理")}</label><input id="browser-proxy" className="browser-input rb-mono" placeholder="http://127.0.0.1:7897" value={draft?.proxy || ''} disabled={locked} onChange={e => edit('proxy', e.currentTarget.value)}/><p className="rb-muted browser-hint">{t("留空不强制启用 TUN：实际连接取决于系统代理、TUN 与路由。指定代理须含端口；保存后重开 B 生效。")}</p></div>
      <div className="rb-row browser-row-wrap"><span className="browser-row-label">{t("语言与时区")}</span><Segment label={t("语言与时区")} value={draft?.follow_machine || false} disabled={locked} onChange={v => edit('follow_machine', v)} options={[{value:true,label:t("跟随电脑")},{value:false,label:t("手动指定")}]}/>
        {draft&&state&&<span className="rb-muted browser-summary" data-browser-config-summary>{t("网页语言配置：{v0} · 网页时区配置：{v1}",{v0:draft.follow_machine?(state.machine.languages[0]==='en'?'en-US':state.machine.languages[0]||'en-US'):draft.language,v1:draft.follow_machine?state.machine.timezone:draft.timezone})}</span>}
        {!draft?.follow_machine && <div className="browser-manual"><Select aria-label={t("浏览器语言")} value={draft?.language} disabled={locked} onValueChange={v => { if (v)
          edit('language', v); }} items={['en-US', 'en-CA', 'en-GB', 'en-SG', 'en-AU', 'en', 'zh-Hans', 'zh-Hans-SG', 'zh-Hant', 'fr', 'de', 'es', 'it', 'ja', 'ko'].map(v => ({ value: v, label: v }))}/><Select aria-label={t("浏览器时区")} value={draft?.timezone} disabled={locked} onValueChange={v => { if (v)
          edit('timezone', v); }} items={(state?.machine.timezones || []).map(v => ({ value: v, label: v }))}/></div>}
        <p className="rb-muted browser-hint">{t("跟随机器时，语言与时区取自系统；关闭跟随后可手动选择。这里只显示配置目标，实际生效值以 B 实测为准。")} {t("修改步骤：正常退出 B → 调整并保存设置 → 由 A 打开浏览器并实测。仅重新打开实测页不会重新加载语言、时区或代理。")}</p>
      </div>
      <fieldset className="rb-row browser-row-wrap browser-dns-settings" disabled={locked}>
       <legend className="browser-row-label">{t('DNS 解析设置')}</legend>
       <Segment label={t('DNS 解析方式')} value={dns.mode} disabled={locked} onChange={v=>editDns({mode:v})} options={[{value:'inherit' as const,label:t('沿用现有解析')},{value:'secure' as const,label:t('指定 DoH')}]}/>
       {dns.mode==='inherit'?<p className="rb-muted browser-hint">{t('保留当前资料原有的安全 DNS。若此前由 A 指定 DoH，下次从 A 启动 B 时恢复接管前设置；原有自动模式可能回退普通 DNS。')}</p>:<div className="browser-doh">
        <Select aria-label={t('DoH 服务商')} value={dns.provider} disabled={locked} onValueChange={v=>{if(v)editDns({provider:v as DnsConfig['provider']});}} items={[...DOH_PROVIDERS.map(p=>({value:p.id,label:p.id==='quad9'?t('Quad9（恶意域名过滤）'):p.name})),{value:'custom',label:t('自定义地址')}]}/>
        {dns.provider==='custom'?<><label htmlFor="browser-doh-url">{t('DoH HTTPS 地址')}</label><input id="browser-doh-url" className="browser-input rb-mono" value={dns.custom_url} maxLength={2048} placeholder="https://resolver.example/dns-query" disabled={locked} aria-invalid={!!dnsError} aria-describedby="browser-doh-input-help" onChange={e=>editDns({custom_url:e.currentTarget.value})}/><p id="browser-doh-input-help" className="rb-muted browser-hint">{t('填写服务商的 DNS wireformat 地址，普通 HTTPS 地址使用 POST；也支持末尾 {?dns} 或 {&dns} 的 GET 模板。保存仅校验格式，不保证服务可用。')}</p></>:<p className="rb-mono browser-hint browser-doh-endpoint">{dohEndpoint(dns)}</p>}
        {dnsError&&<p className="rb-err browser-hint" role="alert">{t(dnsError)}</p>}
        <p className="rb-muted browser-hint">{t('严格 DoH：浏览器自行解析的网站域名在该服务失败时解析失败，不自动回退普通 DNS，也不切换其他服务商。')}</p>
       </div>}
       <p className="rb-muted browser-hint">{t('仅作用于当前身份的 B 浏览器资料；正常退出 B、保存后，从 A 重新启动生效。不会修改系统 DNS、TUN 或代理服务器的 DNS。')}</p>
       <details className="browser-hint"><summary>{t('生效边界与回退')}</summary><p className="rb-muted">{t("指定 DoH 时，A 每次全新启动 B 都会应用此配置；在 B 设置页手动修改，可能在下次启动时被覆盖。切回沿用会恢复 A 首次接管前的两项安全 DNS 设置。")}</p><p className="rb-muted">{t('HTTP/SOCKS 代理代为解析的域名不受浏览器 DoH 控制。DoH 服务自身域名的启动解析可能使用现有 DNS；该选项不强制 DoH 连接走代理，企业策略或家长控制也可能覆盖设置。请以重启后的 DNS 实测和必要的网络日志核对。')}</p></details>
      </fieldset>
      <div className="rb-row"><span className="browser-row-label">{t("设备指纹")}</span><Segment label={t("设备指纹")} value={draft?.native_graphics || false} disabled={locked} onChange={v => edit('native_graphics', v)} options={[{value:true,label:t("使用真实设备指纹")},{value:false,label:t("使用种子生成")}]}/><span className="browser-spacer"/><button type="button" className="rb-btn primary" disabled={locked || !changed || !!dnsError} aria-busy={busy === 'browser_save'} onClick={() => draft && void act('browser_save', { config: draft })}>{t("保存设置")}</button></div>
    </section>
    <section className="rb-card browser-data" aria-labelledby="browser-data-title">
      <h2 id="browser-data-title" className="rb-label">{t("浏览器数据 · 仅当前身份的资料")}</h2>
      <div className="browser-data-row"><div className="browser-data-text"><strong>{formatBytes(cache.reduce((sum, x) => sum + x.bytes, 0))}</strong> <span className="rb-muted">{cache.length}{t(" 个现存项目 · 清理前须退出浏览器")}</span><p className="rb-muted">{t("清理缓存保留登录、书签和密码；全清理会删除登录、密码、书签、历史和扩展，不可恢复。")}</p></div>
        <button type="button" className="rb-btn" disabled={locked || !cache.length || !!state?.cache_error} aria-busy={busy === 'browser_clear'} onClick={() => void act('browser_clear', { includeSiteData: false })}>{t("一键清理缓存")}</button>
        <button type="button" className="rb-btn danger" disabled={locked} aria-busy={busy==='browser_reset'} onClick={() => { setAccepted(false); setConfirm(true); }}>{t("全清理")}</button></div>
      {!!state?.cache.length&&<details className="browser-cache"><summary>{t("查看缓存项目")}</summary>{state.cache.map(x => <div key={x.path}><span className="rb-mono">{x.path}</span><small>{x.login ? t("站点数据") : formatBytes(x.bytes)}</small></div>)}</details>}
      {state?.cache_error && <p role="alert" className="rb-err">{t(state.cache_error)}</p>}
    </section>
    <CleanupActivity activity={activity.state}/>
    <AppDialog alert className="identity-delete-alert" open={confirm} onOpenChange={v => { if (!v) closeConfirm(); }} title={t("全清理浏览器")} description={t("永久删除当前身份的全部本地浏览器资料，不创建备份。")} footer={<><Button variant="secondary" data-dialog-cancel onClick={closeConfirm}>{t("取消")}</Button><Button className="identity-delete-button" variant="ink" disabled={!!busy || !accepted} onClick={() => { if (!accepted) return; closeConfirm(); void act('browser_clear', { includeSiteData: true, resetProfile: true }); }}>{t("确认全清理")}</Button></>}>
      <div className="identity-confirm"><div className="identity-delete-warning"><span aria-hidden="true">!</span><strong>{t("警告：此操作不可撤销，不会创建新备份。")}</strong></div><p>{t("登录状态、密码与密钥、书签、历史、扩展、设置和缓存将全部移除。")}</p><p>{t("其他身份的浏览器资料和身份保险库不受影响。")}</p><Checkbox checked={accepted} onCheckedChange={value => setAccepted(value===true)} label={t("我知道此操作无法撤销")}/></div>
    </AppDialog>
  </section>;
}
