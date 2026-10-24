/**
 * [INPUT]: 随机令牌回环服务上下文、浏览器原生 navigator/Intl/Permissions 与实际 HTTPS 出口与服务备援目录与宿主界面语言。
 * [OUTPUT]: 逐项说明设置位置与生效方式，显示系统区域来源、检测依据与B 触发/A 取数的 DNS 真实结果；单项未知黄色待测与已知冲突红叉； 顶部本地身份工牌与绑定/不匹配/未知状态、全宽紧凑实测起始页，同步宿主中英文界面，显示本机通讯状态与回报时间，观测上报宿主；逐项原因和实际/目标对比，检测中黄底波纹、失败红底、完整通过绿底大对勾。
 * [POS]: 独立 Chromium 入口，复用工具箱控件；成功状态只由真实观测决定，不接入 Tauri 权限，不伪造地理位置。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import { Button, MotionRoot, TooltipProvider } from 'cromma-ui';
import { useEffect, useState } from 'react';
import {setLocale,useI18n} from "./i18n";
import { createRoot } from 'react-dom/client';
import { type BrowserContext, type Report, type Exit } from './browser-model';
import { probeBrowserExit } from './browser-network';
import {verifyBrowser} from './browser-verification';
import {BrowserDns} from './browser-dns-panel';
import browserLogo from '../assets/browser-logo.jpg';
import './index.css';
import './style.css';
const settingHelp:Record<string,string>={
  "出口国家": "在代理工具中选择目标地区的出口，并检查 B 的实际路由；A → 浏览器环境可填写代理地址。修改网页语言或系统区域不会改变出口 IP，切换后重新实测。",
  "网页时区": "A → 浏览器环境：跟随机器时使用系统时区；关闭跟随后可选网页时区。正常退出 B，保存设置，再由 A 启动；刷新页面不会重新加载启动参数。",
  "出口时区": "这是出口 IP 数据库给出的时区，没有独立设置开关。核对代理出口位置；若归属地数据不准确，应核实数据源，不要只为绿勾修改系统时间。",
  "本机时区": "在操作系统的日期与时间设置中修改。当前规则会与网页时区比较；修改系统时区会影响其他应用，跟随机器模式需退出并重开 B。",
  "网页语言": "在 A → 浏览器环境配置。勾选跟随机器时使用系统首选语言；关闭后显示语言选项。正常退出 B → 保存设置 → 由 A 重新启动。A 的中文/EN 按钮只切换界面文案，不改变网页语言。",
  "本机首选语言": "在操作系统的语言设置中调整首选语言顺序。当前规则仍会核对本机语言与浏览器目标语言；只修改浏览器语言不会改变这一项。系统提示重启或注销时按提示操作。",
  "本机区域": "macOS：系统设置 → 通用 → 语言与地区 → 地区。Windows：设置 → 时间和语言 → 语言和区域。Linux：桌面区域与语言设置。这里控制区域格式，不控制 IP 或 DNS；修改后刷新 A 状态并重开 B。",
  "设备平台 / 版本": "由本机系统读取并在 B 启动时传入，没有手动填写入口。若不一致，先确认 B 由当前 A 启动，并在系统更新后重开 B；不要把版本号改成虚构值。",
  "逻辑 CPU": "使用本机实际逻辑核心数，没有手动设置入口。不一致时正常退出 B，再从 A 启动并实测；种子和图形指纹开关不会修改本机核心数。",
  "浏览器实际版本": "与 A 配套的 B 引擎版本比较，没有手动填写入口。若不一致，正常退出旧 B，核对 A/B 是否来自同一安装包，再由 A 启动。",
  "网页定位权限": "A 启动 B 时会将网页定位设为禁止并清除定位例外。若不是 denied，正常退出 B 后由 A 重开；这只影响当前浏览器资料，不会关闭系统定位。",
  "本机定位": "macOS：系统设置 → 隐私与安全性 → 定位服务。Windows：设置 → 隐私和安全性 → 位置。Linux：桌面隐私设置。当前核验要求系统定位关闭；该操作影响其他应用，与网页定位权限分开。"
};
async function api<T>(route: string, body?: unknown): Promise<T> { const response = await fetch(new URL(route, location.href), { signal: AbortSignal.timeout(3000), cache:'no-store', credentials:'omit', redirect:'error', ...(body ? { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) } : {}) }); if (!response.ok)
    throw new Error("本地核验连接失败，请从CCBaga-A重新打开浏览器。"); return response.json(); }
function Start() {
    const {t,locale}=useI18n();
    const [context, setContext] = useState<BrowserContext | null>(null), [report, setReport] = useState<Report | null>(null), [busy, setBusy] = useState(true), [error, setError] = useState(''), [linked, setLinked] = useState(false), [reportedAt, setReportedAt] = useState(0);
    const [dnsRevision,setDnsRevision]=useState(0);
    async function measure() {
        setBusy(true);
        setError('');
        setReport(null);
        setReportedAt(0);
        try {
            const context = await api<BrowserContext>('context');
            setContext(context);setLocale(context.ui_language);
            setLinked(true);
            setDnsRevision(n=>n+1);
            const ua = (navigator as Navigator & {
                userAgentData?: {
                    getHighEntropyValues: (keys: string[]) => Promise<{
                        platformVersion?: string;
                        uaFullVersion?: string;
                    }>;
                };
            }).userAgentData;
            const high = await ua?.getHighEntropyValues(['platformVersion', 'uaFullVersion']).catch(() => null);
            const permission = await navigator.permissions.query({ name: 'geolocation' }).then(x => x.state).catch(() => 'unknown');
            let exit: Exit | null = null, networkError = '';
            try {
                exit = await probeBrowserExit(context.exit_services);
            }
            catch (e) {
                networkError = e instanceof Error ? e.message : String(e);
            }
            const observed: Report = { language: navigator.language, languages: [...navigator.languages], timezone: Intl.DateTimeFormat().resolvedOptions().timeZone, platform: navigator.platform, cores: navigator.hardwareConcurrency, user_agent: navigator.userAgent, platform_version: high?.platformVersion || '', browser_version: high?.uaFullVersion || navigator.userAgent.match(/(?:Chrome|Chromium)\/([\d.]+)/)?.[1] || '', geolocation: permission, exit, error: networkError };
            await api('report', observed);
            setReport(observed);
            setReportedAt(Date.now());
            setLinked(true);
        }
        catch (e) {
            setLinked(false);
            setError(String(e));
        }
        finally {
            setBusy(false);
        }
    }
    useEffect(() => { void measure(); let live = true; const timer = setInterval(() => { void api<BrowserContext>('context').then(next => { if (live) {
        setContext(next);setLocale(next.ui_language);
        setLinked(true);
    } }).catch(() => { if (live)
        setLinked(false); }); }, 10000); return () => { live = false; clearInterval(timer); }; }, []);
    const result=verifyBrowser(context,report,linked,error,busy);
    const {phase,checks,problems}=result;
    const title=phase==='checking'?'检测中':phase==='passed'?'检测通过':'检测未通过';
    const identity=context?.identity;
    const identityName=identity?.name||t(identity?.status==='empty'?'空身份':identity?.status==='unlinked'?'通用浏览器资料':'当前身份');
    const identityStatus=!context&&busy?'正在读取身份':!linked?'主应用未连接':!identity?'身份待核对':identity.status==='unlinked'?'未联动':identity.status==='mismatch'?'身份待同步':identity.status==='unknown'?'身份待核对':'身份已联动';
    const identityReady=linked&&!!identity?.linked&&['linked','draft','empty'].includes(identity.status);

    return <div className={`browser-verification-page is-${phase}`} data-verification={phase}>
     <main className="browser-start">
      <header className="verification-brand"><div><img src={browserLogo} width={28} height={28} alt=""/><strong>CCBaga-B</strong></div><span>Chromium {context?.engine_version||'—'}</span></header>
      <section className={`verification-identity${identityReady?' linked':''}`} aria-label={t('浏览器身份')} data-identity={identity?.status||'unknown'}>
       <div className="verification-identity-avatar" aria-hidden="true">{identity?.status==='empty'?'∅':Array.from(identityName)[0]?.toUpperCase()||'?'}</div>
       <div className="verification-identity-account"><span>Claude · {t('本地身份')}</span><h2>{identityName}</h2><div>{identity?.email&&identity.email!==identityName&&<span>{identity.email}</span>}{identity?.organization&&<span>{identity.organization}</span>}</div></div>
       <span className={`verification-identity-state${identity?.status==='mismatch'||identity?.status==='unknown'?' attention':''}`} role="status">{t(identityStatus)}</span>
      </section>
      <section className="verification-banner" aria-labelledby="verification-title" aria-busy={busy}>
       <div className="verification-signal" aria-hidden="true"><i/><i/><svg viewBox="0 0 96 96"><circle cx="48" cy="48" r="44" fill="currentColor"/>{phase==='passed'?<path d="m27 49 14 14 28-31"/>:phase==='failed'?<path d="m34 34 28 28m0-28L34 62"/>:<><path d="M48 27v23"/><circle cx="48" cy="65" r="3" fill="white" stroke="none"/></>}</svg></div>
       <div className="verification-title" aria-live="polite"><span>{t('浏览器环境')}</span><h1 id="verification-title">{t(title)}</h1>{!busy&&<span>{t('已通过 {v0} / {v1} 项',{v0:checks.filter(check=>check.ok).length,v1:checks.length})}</span>}</div>
       <Button variant="ink" loading={busy} disabled={busy} onClick={()=>void measure()}>{t('重新实测')}</Button>
      </section>
      <div className="landing-section-band" aria-hidden="true"/>
      {context&&<section className="verification-network" aria-label={t('检测依据')}><div><span>{t('当前模式')}</span><strong>{t(context.config.follow_machine?'跟随本机':'手动配置')}</strong></div><div><span>{t('目标地区')}</span><strong>{context.region?`${context.region.label} (${context.region.country})`:t('尚未识别')}</strong></div></section>}
      {phase==='failed'&&problems.length>0&&<section className="verification-reasons" aria-labelledby="verification-reasons-title"><h2 id="verification-reasons-title">{t('未通过原因')}</h2><ul>{problems.map(problem=><li key={problem}>{t(problem)}</li>)}</ul></section>}
      {checks.length>0&&<section className="verification-results" aria-label={t('逐项检测结果')}>{checks.map(check=><article className={`verification-check ${check.ok==null?'pending':check.ok?'ok':'failed'}`} key={check.label}><header><h2>{t(check.label)}</h2><span aria-label={t(check.ok==null?'尚未核验':check.ok?'检测通过':'检测未通过')}>{check.ok==null?t('尚未核验'):check.ok?'✓':'×'}</span></header><strong>{t(check.actual)}</strong><p>{t('目标：{v0}',{v0:t(check.expected)})}</p>{check.label==='本机区域'&&<p className="verification-source">{t(context?.machine.platform==='macos'?'来源：macOS 系统设置 → 通用 → 语言与地区 → 地区。':'来源：操作系统的区域设置。')}{context?.machine.locale_country&&` ${t('区域代码：{v0}',{v0:context.machine.locale_country})}`}{' '}{t('这是系统区域格式，不是 IP 所在地。')}{check.ok==null&&` ${t(!context?.region?'目标地区未识别，暂不判断是否一致。':'系统区域未读到，暂不判断是否一致。')}`}</p>}<details className="verification-setting-help"><summary>{t('如何设置')}</summary><p>{t(settingHelp[check.label])}</p></details></article>)}</section>}
      {phase==='checking'&&<div className="verification-loading" role="status">{t('正在读取浏览器与网络…')}</div>}
      {report?.exit&&<section className="verification-network"><div><span>{t('出口 IP')}</span><strong>{report.exit.ip}</strong></div><div><span>{t('国家 / 城市')}</span><strong>{report.exit.country} / {report.exit.city||'—'}</strong></div></section>}
      <BrowserDns revision={dnsRevision} connected={linked} config={context?.config}/>
      <footer className="verification-footer"><span className={linked?'connected':'disconnected'}>{t(linked?'已连接 CCBaga-A':'主应用未连接')}</span><time>{reportedAt?t('结果回传成功 {v0}',{v0:new Date(reportedAt).toLocaleTimeString(locale==='zh'?'zh-CN':'en-US')}):t('等待检测回传')}</time></footer>
     </main>
    </div>;

}
createRoot(document.getElementById('app')!).render(<MotionRoot><TooltipProvider><Start /></TooltipProvider></MotionRoot>);
