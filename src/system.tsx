/**
 * [INPUT]: rb.css 共享类、Cromma Input/Select、原生电脑环境快照（时区/语言/区域格式/定位）、get_regions 13 项地区目录、浏览器指纹命令与共享 YAML 文案。
 * [OUTPUT]: 电脑环境页：顶部「一键按地区设置」（地区胶囊、将设置的时区·语言·区域格式、可选关闭定位与同步浏览器指纹、逐步真实记录且失败即停）；下方当前设置逐项修改（时区/首选语言行内编辑、区域格式只读、定位关闭）；页尾系统授权与钥匙串（Windows 隐藏）文字链。
 * [POS]: 系统设置的交互边界；草稿与当前状态分离，页面加载不修改电脑，窗口聚焦时读回；部分完成如实报告已完成的步骤。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import { Input, Select } from 'cromma-ui';
import { useEffect, useRef, useState } from 'react';
import {useI18n} from "./i18n";
import { invoke } from '@tauri-apps/api/core';
import type { BrowserState } from './browser-model';
import { CleanupActivity, useCleanupActivity } from './cleaner-activity';
type SystemState = {
    platform?: string;
    timezone: string;
    timezones: string[];
    automatic_timezone: boolean | null;
    languages: string[];
    locale?: string;
    location_enabled: boolean | null;
    accessibility: boolean;
};
type SystemResult = {
    state: SystemState;
    message: string;
};
export type Region = { id: string; label: string; country: string; language: string; locale: string; timezone: string };
const languages: Record<string, string> = { 'zh-Hans': '简体中文', 'zh-Hant': '繁體中文', en: 'English', fr: 'Français', de: 'Deutsch', es: 'Español', it: 'Italiano', 'pt-BR': 'Português (Brasil)', ja: '日本語', ko: '한국어', ru: 'Русский', ar: 'العربية' };
function languageChoice(code: string) { return Object.keys(languages).find(key => code === key || code.startsWith(key + '-')); }
// ----- 三平台格式不同（en-US / en_US.UTF-8 / en_US@rg=…），只比较语言标签与区域标识本身 -----
const tag = (value: string) => value.split(/[.@]/)[0].replaceAll('_', '-');
export function languageMatches(current: string | undefined, target: string) { if (!current) return false; const value = tag(current); return value === target || value.startsWith(target + '-'); }
export function localeMatches(current: string | undefined, target: string) { return !!current && tag(current) === tag(target); }
function offset(zone: string) {
    try {
        const part = new Intl.DateTimeFormat('en-US', { timeZone: zone, timeZoneName: 'shortOffset' }).formatToParts(new Date()).find(p => p.type === 'timeZoneName')?.value;
        return part ? part.replace('GMT', 'UTC').replace(/^UTC$/, 'UTC+0') : '';
    }
    catch { return ''; }
}
type Busy = 'read' | 'region' | 'timezone' | 'language' | 'location' | 'keychain';
export function SystemPanel({ onBusy }: {
    onBusy?: (busy: boolean) => void;
}) {
    const {t}=useI18n();
    const [state, setState] = useState<SystemState | null>(null);
    const [regions, setRegions] = useState<Region[]>([]);
    const [regionId, setRegionId] = useState('');
    const [alsoLocation, setAlsoLocation] = useState(true);
    const [syncBrowser, setSyncBrowser] = useState(false);
    const [editing, setEditing] = useState<'timezone' | 'language' | null>(null);
    const [zone, setZone] = useState('');
    const [language, setLanguage] = useState('en');
    const [filter, setFilter] = useState('');
    const [busy, setBusy] = useState<Busy | null>(null);
    const [error, setError] = useState('');
    const busyRef = useRef(false);
    const live = useRef(true);
    const chosen = useRef(false);
    const activity = useCleanupActivity();
    useEffect(() => { onBusy?.(busy !== null); return () => onBusy?.(false); }, [busy, onBusy]);
    async function refresh() {
        const next = await invoke<SystemState>('system_read');
        if (live.current)
            setState(next);
        return next;
    }
    useEffect(() => {
        live.current = true;
        void refresh().catch(e => { if (live.current)
            setError(String(e)); });
        void invoke<Region[]>('get_regions').then(list => { if (live.current)
            setRegions(list); }).catch(e => { if (live.current)
            setError(String(e)); });
        const onFocus = () => { if (!busyRef.current)
            void refresh().catch(e => setError(String(e))); };
        window.addEventListener('focus', onFocus);
        return () => { live.current = false; window.removeEventListener('focus', onFocus); };
    }, []);
    // 初次读到快照后预选与当前电脑最接近的地区；用户点选后不再覆盖。
    useEffect(() => {
        if (chosen.current || !regions.length || !state)
            return;
        const best = regions.find(r => r.timezone === state.timezone && languageMatches(state.languages[0], r.language)) || regions.find(r => r.timezone === state.timezone) || regions[0];
        setRegionId(best.id);
    }, [regions, state]);
    const region = regions.find(r => r.id === regionId);
    function lock(kind: Busy) {
        if (busyRef.current)
            return false;
        busyRef.current = true;
        setBusy(kind);
        setError('');
        return true;
    }
    function unlock() { busyRef.current = false; setBusy(null); }
    async function applyRegion() {
        if (!region || !lock('region'))
            return;
        const target = region;
        activity.start('system', '按地区设置记录');
        let step = 'region';
        let current = state;
        try {
            activity.log('system', 'running', `正在设置时区、语言与区域格式：${target.timezone} · ${target.language} · ${target.locale}`);
            const applied = await invoke<SystemResult>('system_set_region', { region: target.id });
            current = applied.state;
            setState(applied.state);
            activity.log('system', 'ok', applied.message);
            if (alsoLocation) {
                step = 'location';
                if (current.location_enabled === false)
                    activity.log('system', 'ok', '定位已是关闭状态，跳过。');
                else {
                    activity.log('system', 'running', '正在关闭定位；需要时请完成系统设置授权。');
                    const located = await invoke<SystemResult>('system_disable_location');
                    current = located.state;
                    setState(located.state);
                    activity.log('system', located.state.location_enabled === false ? 'ok' : 'warning', located.message);
                }
            }
            if (syncBrowser) {
                step = 'browser';
                const before = await invoke<BrowserState>('browser_read');
                activity.log('browser', 'running', '正在更换浏览器指纹并写入地区语言与时区。');
                await invoke('browser_rotate', { region: target.id });
                activity.log('browser', 'ok', `浏览器指纹已更换，语言与时区：${target.language} · ${target.timezone}`);
                if (before.running) {
                    step = 'launch';
                    await invoke('browser_launch', { onProgress: activity.channel('browser') });
                    activity.log('browser', 'ok', 'B 浏览器已用新指纹重新启动。');
                }
                else
                    activity.log('browser', 'ok', 'B 浏览器未在运行，新指纹在下次启动时生效。');
            }
            activity.finish(true, `已按「${target.label}」完成设置。`);
        }
        catch (e) {
            const message = String(e);
            setError(message);
            activity.log(step === 'browser' || step === 'launch' ? 'browser' : 'system', 'error', message);
            activity.finish(false, step === 'region' ? '按地区设置失败，后续步骤未执行。' : step === 'location' ? (syncBrowser ? '时区、语言与区域格式已设置；关闭定位失败，浏览器指纹未同步。' : '时区、语言与区域格式已设置；关闭定位失败。') : step === 'browser' ? '电脑设置已完成；浏览器指纹同步失败。' : '电脑设置已完成，新指纹已保存；B 浏览器重新启动失败，请到浏览器环境页手动启动。');
            try {
                await refresh();
            }
            catch {
                if (!current)
                    setState(null);
            }
        }
        finally {
            unlock();
        }
    }
    async function apply(kind: 'timezone' | 'language' | 'location') {
        if (!lock(kind))
            return;
        activity.start('system');
        activity.log('system', 'running', kind === 'timezone' ? `正在应用 ${zone}，请完成系统管理员授权。` : kind === 'language' ? `正在保存首选语言：${languages[language]}。` : "正在关闭定位；需要时请完成系统设置授权。");
        try {
            const result = await invoke<SystemResult>(kind === 'timezone' ? 'system_set_timezone' : kind === 'language' ? 'system_set_language' : 'system_disable_location', kind === 'timezone' ? { timezone: zone } : kind === 'language' ? { language } : {});
            setState(result.state);
            const confirmed = kind !== 'location' || result.state.location_enabled === false;
            activity.log('system', confirmed ? 'ok' : 'warning', result.message);
            activity.finish(confirmed, result.message);
            if (kind !== 'location')
                setEditing(null);
        }
        catch (e) {
            const message = String(e);
            setError(message);
            activity.log('system', 'error', message);
            activity.finish(false, message);
            try {
                await refresh();
            }
            catch {
                setState(null);
            }
        }
        finally {
            unlock();
        }
    }
    async function reload() {
        if (!lock('read'))
            return;
        try {
            await refresh();
        }
        catch (e) {
            setState(null);
            setError(String(e));
        }
        finally {
            unlock();
        }
    }
    async function unlockKeychain() {
        if (!lock('keychain'))
            return;
        activity.start('system');
        try {
            activity.finish(true, await invoke<string>('unlock_keychain'));
        }
        catch (e) {
            setError(String(e));
            activity.finish(false, String(e));
        }
        finally {
            unlock();
        }
    }
    async function open(pane: string) { try {
        await invoke('system_open_settings', { pane });
    }
    catch (e) {
        setError(String(e));
    } }
    function edit(kind: 'timezone' | 'language') {
        if (!state)
            return;
        setFilter('');
        if (kind === 'timezone')
            setZone(region && state.timezones.includes(region.timezone) ? region.timezone : state.timezone);
        else
            setLanguage((region && languageChoice(region.language)) || languageChoice(state.languages[0] || 'en') || 'en');
        setEditing(editing === kind ? null : kind);
    }
    const normalizedFilter = filter.trim().toLowerCase().replaceAll('_', ' ');
    const zones = state ? state.timezones.filter(value => value === zone || value.toLowerCase().replaceAll('_', ' ').includes(normalizedFilter)) : [];
    const disabled = busy !== null || !state;
    const windows = state?.platform === 'windows';
    const match = (ok: boolean) => !region || !state ? null : ok ? <span className="rb-muted rb-ok">{t("与所选地区一致")}</span> : <span className="rb-muted rb-warn">{t("与所选地区不一致")}</span>;
    const zoneOffset = state ? offset(state.timezone) : '';
    return <section className="rb-page system-rb">
    <header><h1 className="rb-h1">{t("电脑环境")}</h1><p className="rb-sub">{t("选一个地区，一次改好时区、语言和区域格式；也可以在下面逐项改。")}</p></header>
    {error && <p className="rb-muted rb-err system-rb-error" role="alert">{t(error)}</p>}
    <div className="rb-card">
      <p className="rb-label">{t("一键按地区设置")}</p>
      <div className="system-rb-regions" role="group" aria-label={t("国家地区")}>{regions.map(r => <button key={r.id} type="button" className="rb-chip" aria-pressed={r.id === regionId} title={t(r.label)} disabled={busy !== null} onClick={() => { chosen.current = true; setRegionId(r.id); }}><span>{t(r.label)}</span></button>)}</div>
      <div className="system-rb-apply">
        <div className="system-rb-target">{t("将设置为")} <span className="rb-mono">{region ? `${region.timezone} · ${region.language} · ${region.locale}` : '—'}</span></div>
        <label className="rb-check"><input type="checkbox" checked={alsoLocation} disabled={busy !== null} onChange={e => setAlsoLocation(e.currentTarget.checked)}/>{t("同时关闭定位")}</label>
        <label className="rb-check"><input type="checkbox" checked={syncBrowser} disabled={busy !== null} onChange={e => setSyncBrowser(e.currentTarget.checked)}/>{t("同步浏览器指纹")}</label>
        <button type="button" className="rb-btn primary" disabled={disabled || !region} aria-busy={busy === 'region'} onClick={() => void applyRegion()}>{busy === 'region' ? t("正在应用…") : t("应用到本机")}</button>
      </div>
    </div>
    <CleanupActivity activity={activity.state}/>
    <div className="rb-card line system-rb-current">
      <div className="system-rb-current-head"><p className="rb-label">{t("当前设置 · 可逐项修改")}</p><button type="button" className="rb-link" disabled={busy !== null} onClick={() => void reload()}>{busy === 'read' ? t("读取中…") : t("刷新状态")}</button></div>
      <div className="rb-row"><span className="k">{t("时区")}</span><span className="v rb-mono">{state ? `${state.timezone}${zoneOffset ? `（${zoneOffset}）` : ''}` : t("读取中…")}</span>{state && match(state.timezone === region?.timezone)}<button type="button" className="rb-link" aria-expanded={editing === 'timezone'} disabled={disabled} onClick={() => edit('timezone')}>{editing === 'timezone' ? t("收起") : t("修改")}</button></div>
      {editing === 'timezone' && state && <div className="system-rb-edit">
        <Input size="small" aria-label={t("查找时区")} value={filter} onChange={event => setFilter(event.currentTarget.value)} placeholder={t("例如 America / Paris / Shanghai")} disabled={disabled}/>
        <Select aria-label={t("目标时区")} size="small" value={zone || undefined} onValueChange={value => { if (value)
            setZone(value); }} items={zones.map(value => ({ value, label: value.replaceAll('_', ' ') }))} disabled={disabled}/>
        <button type="button" className="rb-btn" disabled={disabled || !zone || (zone === state.timezone && state.automatic_timezone === false)} aria-busy={busy === 'timezone'} onClick={() => void apply('timezone')}>{t("应用时区")}</button>
        <button type="button" className="rb-link" disabled={busy !== null} onClick={() => void open('timezone')}>{t("系统设置")}</button>
        <p className="rb-muted">{state.automatic_timezone === null ? t("自动时区状态未知") : state.automatic_timezone ? t("按位置自动设置：开启") : t("手动时区")} · {t(windows ? "需要时请完成系统授权。" : "需要管理员授权。")}</p>
      </div>}
      <div className="rb-row"><span className="k">{t("首选语言")}</span><span className="v rb-mono" title={state?.languages.join(' → ')}>{state ? state.languages[0] || t("未设置") : t("读取中…")}</span>{state && region && match(languageMatches(state.languages[0], region.language))}<button type="button" className="rb-link" aria-expanded={editing === 'language'} disabled={disabled} onClick={() => edit('language')}>{editing === 'language' ? t("收起") : t("修改")}</button></div>
      {editing === 'language' && state && <div className="system-rb-edit">
        <Select aria-label={t("目标语言")} size="small" value={language} onValueChange={value => { if (value)
            setLanguage(value); }} items={Object.entries(languages).map(([value, label]) => ({ value, label: t(label) }))} disabled={disabled}/>
        <button type="button" className="rb-btn" disabled={disabled || language === languageChoice(state.languages[0] || 'en')} aria-busy={busy === 'language'} onClick={() => void apply('language')}>{t("应用语言")}</button>
        <button type="button" className="rb-link" disabled={busy !== null} onClick={() => void open('language')}>{t("系统设置")}</button>
        <p className="rb-muted">{t("应用重启后生效；系统完整切换可能需要注销。")}</p>
      </div>}
      <div className="rb-row"><span className="k">{t("区域格式")}</span><span className="v rb-mono">{!state ? t("读取中…") : state.locale || t("未知")}</span>{state?.locale && region ? match(localeMatches(state.locale, region.locale)) : null}<span className="rb-muted">{t("随地区一键设置")}</span></div>
      <div className="rb-row"><span className="k">{t("定位服务")}</span><span className="v">{!state ? t("读取中…") : state.location_enabled === null ? t("尚未核验") : state.location_enabled ? t("已开启") : t("已关闭")}{state?.location_enabled && !windows && !state.accessibility ? <span className="rb-muted system-rb-note">{t("需要辅助功能权限，或在系统设置中手动关闭。")}</span> : null}</span><button type="button" className="rb-link" disabled={busy !== null} onClick={() => void open('location')}>{t("系统设置")}</button><button type="button" className="rb-btn" disabled={disabled || state?.location_enabled === false} aria-busy={busy === 'location'} onClick={() => void apply('location')}>{state?.location_enabled === false ? t("已关闭") : t("关闭")}</button></div>
    </div>
    <footer className="system-rb-footer">
      <button type="button" className="rb-link" disabled={busy !== null} onClick={async () => { try {
            await invoke('open_permissions');
        }
        catch (e) {
            setError(String(e));
        } }}>{t("系统授权设置")}</button>
      {!windows && <button type="button" className="rb-link" disabled={busy !== null} aria-busy={busy === 'keychain'} onClick={() => void unlockKeychain()}>{t("解锁钥匙串")}</button>}
      <span className="system-rb-spacer"/>
      <span className="rb-muted">{t(windows ? "需要时请完成系统授权。" : "改时区需要管理员密码；改语言后需重开应用。")}</span>
    </footer>
  </section>;
}
