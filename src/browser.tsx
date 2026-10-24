/**
 * [INPUT]: Cromma UI、原生浏览器命令、带回报时间的真实观测事件、共用地区契约与共享活动日志和 YAML 文案。
 * [OUTPUT]: 支持 zh-Hans-SG、两模式 DNS 设置及作用范围，常显语言/时区配置目标与保存重启说明，当前身份的指纹配置、独立浏览器启动、仅缓存/当前身份全部本地资料清理、目标地区与机器一致性核验、明确的 B 浏览器运行/身份联动状态与 CCBaga-B 一键设为默认浏览器（系统确认）。
 * [POS]: 浏览器交互边界；只展示配置、操作与真实观测，页面加载只读，系统对齐和登录删除必须由用户选择；全清理使用默认取消且需勾选确认的 AlertDialog，不新增备份，不影响其他身份资料。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import { Button, Input, Select, Checkbox, Panel, PanelHeader } from 'cromma-ui';
import { useEffect, useRef, useState } from 'react';
import {useI18n} from "./i18n";
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { AppDialog } from './dialog';
import { CleanupActivity, useCleanupActivity } from './cleaner-activity';
import type { BrowserConfig, BrowserState, DefaultBrowser } from './browser-model';
import {DEFAULT_DNS,DOH_PROVIDERS,dohEndpoint,validateDoh,type DnsConfig} from './browser-doh';
import { formatBytes } from './cleaner-model';
export function BrowserPanel({ onBusy }: {
    onBusy?: (busy: boolean) => void;
}) {
    const {t,locale}=useI18n();
    const [state, setState] = useState<BrowserState | null>(null), [draft, setDraft] = useState<BrowserConfig | null>(null), [busy, setBusy] = useState(''), [error, setError] = useState(''), [confirm, setConfirm] = useState<'reset' | 'align' | null>(null);
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
        setDefaultNote(next.is_default ? "CCBaga-B已是默认浏览器。" : "已向系统请求设为默认浏览器，请检查系统确认。");
        for (let i = 0; i < 20 && live.current; i++) {
            await new Promise(r => setTimeout(r, 1500));
            const fresh = await readDefault();
            if (fresh?.is_default) {
                setDefaultNote("CCBaga-B已设为默认浏览器。");
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
        const next = await invoke<BrowserState>(command, { ...args, ...(['browser_launch', 'browser_clear', 'browser_align'].includes(command) ? { onProgress } : {}) });
        setState(next);
        if (command === 'browser_save' || command === 'browser_align')
            setDraft(next.config);
        activity.finish(true, command === 'browser_clear' ? (args.resetProfile ? "当前身份的浏览器资料已全清理，未保留副本。" : "清理完成，未保留副本。") : command === 'browser_check' ? "机器出口已检测；浏览器结果由实测页上报。" : "操作完成。");
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
    const changed = state && draft && JSON.stringify(state.config) !== JSON.stringify(draft);
    const cache = state?.cache.filter(x => !x.login) || [];
    function edit<K extends keyof BrowserConfig>(key: K, value: BrowserConfig[K]) { setDraft(old => old ? { ...old, [key]: value } : old); }
    return <section className="system-page browser-page"><div className="cleanup-title"><div><h1>{t("浏览器环境")}</h1></div><Button size="small" variant="secondary" disabled={!!busy} onClick={() => void act('browser_read')}>{t("刷新状态")}</Button></div>
    <div className="browser-default"><strong>{state?.running ? t("B 浏览器运行中") : t("B 浏览器未运行")}{state?.identity_linked&&<> · {t("浏览器资料随身份切换")}</>}</strong><Button variant="secondary" size="small" disabled={!!busy || asking || !preferred?.installed || preferred?.is_default} loading={asking} onClick={() => void makeDefault()}>{preferred?.is_default ? t("已是默认浏览器") : t("设为默认浏览器")}</Button></div>
    {defaultNote && <p className="system-feedback" role="status">{t(defaultNote)}</p>}
    {error && <p role="alert" className="system-feedback error">{t(error)}</p>}
    <div className="browser-actions"><Button variant="ink" disabled={disabled || !state?.engine_ready || !!changed} loading={busy === 'browser_launch'} onClick={() => void act('browser_launch')}>{state?.running ? t("重新打开实测页") : t("打开浏览器并实测")}</Button><Button variant="secondary" disabled={disabled || !state?.running} loading={busy === 'browser_quit'} onClick={() => void act('browser_quit')}>{t("正常退出浏览器")}</Button><Button variant="secondary" disabled={disabled} loading={busy === 'browser_check'} onClick={() => void act('browser_check')}>{t("检测机器出口")}</Button><Button variant="ghost" disabled={disabled || state?.running || !!changed} onClick={() => setConfirm('align')}>{t("对齐美国系统环境")}</Button></div>
    {!!changed && <p className="system-hint">{t("先保存指纹设置，再启动浏览器或对齐机器。")}</p>}
    <div className="browser-grid"><Panel className="system-card"><PanelHeader title={t("指纹与代理设置")}/><div className="system-card-body">
      <label htmlFor="fingerprint-seed">{t("稳定指纹种子")}</label><Input id="fingerprint-seed" type="number" min={1} max={4294967295} value={draft?.seed ?? ''} disabled={disabled || state?.running} onChange={e => edit('seed', Number(e.currentTarget.value))}/>
      <label htmlFor="browser-proxy">{t("浏览器代理")}</label><Input id="browser-proxy" placeholder="http://127.0.0.1:7897" value={draft?.proxy || ''} disabled={disabled || state?.running} onChange={e => edit('proxy', e.currentTarget.value)}/><p className="system-hint">{t("留空不强制启用 TUN：实际连接取决于系统代理、TUN 与路由。指定代理须含端口；保存后重开 B 生效。")}</p>
      <label className="browser-toggle"><Checkbox checked={draft?.follow_machine || false} disabled={disabled || state?.running} onCheckedChange={v => edit('follow_machine', v === true)}/>{t("语言与时区跟随机器")}</label>
      {draft&&state&&<p className="system-hint" data-browser-config-summary>{t("网页语言配置：{v0} · 网页时区配置：{v1}",{v0:draft.follow_machine?(state.machine.languages[0]==='en'?'en-US':state.machine.languages[0]||'en-US'):draft.language,v1:draft.follow_machine?state.machine.timezone:draft.timezone})}</p>}
      <p className="system-hint">{t("跟随机器时，语言与时区取自系统；关闭跟随后可手动选择。这里只显示配置目标，实际生效值以 B 实测为准。")}</p><p className="system-hint">{t("修改步骤：正常退出 B → 调整并保存设置 → 由 A 打开浏览器并实测。仅重新打开实测页不会重新加载语言、时区或代理。")}</p>
      {!draft?.follow_machine && <><label>{t("网页语言")}</label><Select aria-label={t("浏览器语言")} value={draft?.language} disabled={disabled || state?.running} onValueChange={v => { if (v)
        edit('language', v); }} items={['en-US', 'en-CA', 'en-GB', 'en-SG', 'en-AU', 'en', 'zh-Hans', 'zh-Hans-SG', 'zh-Hant', 'fr', 'de', 'es', 'it', 'ja', 'ko'].map(v => ({ value: v, label: v }))}/><label>{t("网页时区")}</label><Select aria-label={t("浏览器时区")} value={draft?.timezone} disabled={disabled || state?.running} onValueChange={v => { if (v)
        edit('timezone', v); }} items={(state?.machine.timezones || []).map(v => ({ value: v, label: v }))}/></>}
      <fieldset className="browser-dns-settings" disabled={disabled||!!state?.running}>
       <legend>{t('DNS 解析设置')}</legend>
       <Select aria-label={t('DNS 解析方式')} value={dns.mode} disabled={disabled||!!state?.running} onValueChange={v=>{if(v==='inherit'||v==='secure')editDns({mode:v});}} items={[{value:'inherit',label:t('沿用现有解析')},{value:'secure',label:t('指定 DoH')}]}/>
       {dns.mode==='inherit'?<p className="system-hint">{t('保留当前资料原有的安全 DNS。若此前由 A 指定 DoH，下次从 A 启动 B 时恢复接管前设置；原有自动模式可能回退普通 DNS。')}</p>:<>
        <label>{t('DoH 服务商')}</label><Select aria-label={t('DoH 服务商')} value={dns.provider} disabled={disabled||!!state?.running} onValueChange={v=>{if(v)editDns({provider:v as DnsConfig['provider']});}} items={[...DOH_PROVIDERS.map(p=>({value:p.id,label:p.id==='quad9'?t('Quad9（恶意域名过滤）'):p.name})),{value:'custom',label:t('自定义地址')}]}/>
        {dns.provider==='custom'?<><label htmlFor="browser-doh-url">{t('DoH HTTPS 地址')}</label><Input id="browser-doh-url" value={dns.custom_url} maxLength={2048} placeholder="https://resolver.example/dns-query" disabled={disabled||!!state?.running} aria-invalid={!!dnsError} aria-describedby="browser-doh-input-help" onChange={e=>editDns({custom_url:e.currentTarget.value})}/><p id="browser-doh-input-help" className="system-hint">{t('填写服务商的 DNS wireformat 地址，普通 HTTPS 地址使用 POST；也支持末尾 {?dns} 或 {&dns} 的 GET 模板。保存仅校验格式，不保证服务可用。')}</p></>:<p className="system-hint browser-doh-endpoint">{dohEndpoint(dns)}</p>}
        {dnsError&&<p className="system-feedback error" role="alert">{t(dnsError)}</p>}
        <p className="system-hint">{t('严格 DoH：浏览器自行解析的网站域名在该服务失败时解析失败，不自动回退普通 DNS，也不切换其他服务商。')}</p>
       </>}
       <p className="system-hint">{t('仅作用于当前身份的 B 浏览器资料；正常退出 B、保存后，从 A 重新启动生效。不会修改系统 DNS、TUN 或代理服务器的 DNS。')}</p>
       <details><summary>{t('生效边界与回退')}</summary><p className="system-hint">{t("指定 DoH 时，A 每次全新启动 B 都会应用此配置；在 B 设置页手动修改，可能在下次启动时被覆盖。切回沿用会恢复 A 首次接管前的两项安全 DNS 设置。")}</p><p className="system-hint">{t('HTTP/SOCKS 代理代为解析的域名不受浏览器 DoH 控制。DoH 服务自身域名的启动解析可能使用现有 DNS；该选项不强制 DoH 连接走代理，企业策略或家长控制也可能覆盖设置。请以重启后的 DNS 实测和必要的网络日志核对。')}</p></details>
      </fieldset>
      <label className="browser-toggle"><Checkbox checked={draft?.native_graphics || false} disabled={disabled || state?.running} onCheckedChange={v => edit('native_graphics', v === true)}/>{t("使用真实设备指纹")}</label>
      <div className="system-card-actions"><Button variant="ink" size="small" disabled={disabled || state?.running || !changed || !!dnsError} loading={busy === 'browser_save'} onClick={() => draft && void act('browser_save', { config: draft })}>{t("保存设置")}</Button></div>
    </div></Panel><Panel className="system-card"><PanelHeader title={t("浏览器数据")}/><div className="system-card-body"><div className="system-current"><span>{t("可直接清理缓存")}</span><strong>{formatBytes(cache.reduce((sum, x) => sum + x.bytes, 0))}</strong><small>{cache.length}{t(" 个现存项目 · 清理前须退出浏览器")}</small></div>
      <div className="browser-cache">{state?.cache.length ? state.cache.map(x => <div key={x.path}><span>{x.path}</span><small>{x.login ? t("站点数据") : formatBytes(x.bytes)}</small></div>) : <p>{t("未发现缓存项目。")}</p>}</div>{state?.cache_error && <p role="alert">{t(state.cache_error)}</p>}
      <div className="browser-actions"><Button variant="ink" size="small" disabled={disabled || state?.running || !cache.length || !!state?.cache_error} loading={busy === 'browser_clear'} onClick={() => void act('browser_clear', { includeSiteData: false })}>{t("一键清理缓存")}</Button><Button variant="secondary" size="small" disabled={disabled || state?.running} loading={busy==='browser_reset'} onClick={() => { setAccepted(false); setConfirm('reset'); }}>{t("全清理")}</Button></div>
    </div></Panel></div>
    <Panel className="browser-checks"><PanelHeader title={t("{v0} · 实际观测", { v0: t(state?.region?.label || "自定义环境") })} actions={<span className="system-card-number">{state?.report_received_at ? t("已连接 · 回报 {v0}", { v0: new Date(state.report_received_at).toLocaleTimeString(locale==='zh'?'zh-CN':'en-US') }) : state?.running ? t("等待浏览器回报") : t("浏览器未运行")}</span>}/><div className="browser-check-grid">{state?.checks.map(x => <div className={`browser-check check-${x.status}`} key={t(x.title)}><span aria-label={t(x.status==='ok'?"已核验":x.status==='mismatch'?"存在冲突":"尚未核验")}>{x.status === 'ok' ? '✓' : x.status === 'mismatch' ? '!' : '—'}</span><div><strong>{t(x.title)}</strong><p>{t(x.detail)}</p></div></div>)}</div>{state?.report?.error && <p className="system-feedback error">{t("浏览器出口检测：")}{t(state.report.error)}</p>}</Panel>
    <CleanupActivity activity={activity.state}/>
    <AppDialog alert={confirm==='reset'} className={confirm==='reset'?'identity-delete-alert':undefined} open={confirm !== null} onOpenChange={v => { if (!v) { setConfirm(null); setAccepted(false); } }} title={confirm === 'reset' ? t("全清理浏览器") : t("对齐美国系统环境")} description={confirm === 'reset' ? t("永久删除当前身份的全部本地浏览器资料，不创建备份。") : t("先核验代理实际在美国，再将机器时区设为出口时区、首选语言设为 en-US、区域设为 en_US，并关闭系统定位。时区可能需要管理员授权。")} footer={<><Button variant="secondary" data-dialog-cancel onClick={() => { setConfirm(null); setAccepted(false); }}>{t("取消")}</Button><Button className={confirm==='reset'?'identity-delete-button':undefined} variant="ink" disabled={!!busy || (confirm==='reset'&&!accepted)} onClick={() => { const action = confirm; if (action==='reset'&&!accepted) return; setConfirm(null); setAccepted(false); void act(action === 'reset' ? 'browser_clear' : 'browser_align', action === 'reset' ? { includeSiteData: true, resetProfile: true } : {}); }}>{confirm === 'reset' ? t("确认全清理") : t("检测并应用")}</Button></>}>
      {confirm==='reset'?<div className="identity-confirm"><div className="identity-delete-warning"><span aria-hidden="true">!</span><strong>{t("警告：此操作不可撤销，不会创建新备份。")}</strong></div><p>{t("登录状态、密码与密钥、书签、历史、扩展、设置和缓存将全部移除。")}</p><p>{t("其他身份的浏览器资料和身份保险库不受影响。")}</p><Checkbox checked={accepted} onCheckedChange={value => setAccepted(value===true)} label={t("我知道此操作无法撤销")}/></div>:<p>{t("不会修改代理节点。若机器与浏览器出口不同，需在代理工具中统一系统路由。更改系统语言后，部分应用需重新启动或注销。")}</p>}
    </AppDialog>
  </section>;
}
