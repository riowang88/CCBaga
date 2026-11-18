/**
 * [INPUT]: rb.css 奶油/珊瑚设计基础、Cromma UI 运行时、共享 YAML 界面语言、电脑/网络/身份/浏览器/说明页面与官方签名更新组件。
 * [OUTPUT]: 奶油侧栏（--rb-surface-soft 底、选中项 --rb-cream-strong）：电脑环境 → 网络环境 → Claude 身份 → 浏览器环境 ｜分隔｜ 使用说明；底部当前版本号、检查更新与语言切换。
 * [POS]: 单窗口编排层；默认打开电脑环境；高级页面或更新运行时锁定导航，页面各自在聚焦/完成后读回快照。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import { MotionRoot, TooltipProvider } from 'cromma-ui';
import { Fragment, useState } from 'react';
import {useI18n} from "./i18n";
import { createRoot } from 'react-dom/client';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { IdentityPanel } from './identities';
import { BrowserPanel } from './browser';
import { SystemPanel } from './system';
import { VpsPanel } from './vps';
import { LanguageSwitch } from './language-switch';
import { AppUpdate } from './app-update';
import { HelpPanel } from './help';
import './index.css';
import './style.css';
type Page = 'system' | 'vps' | 'cleaner' | 'browser' | 'help';
const order: readonly Page[] = ['system', 'vps', 'cleaner', 'browser', 'help'];
const titles: Record<Page, string> = { system: '电脑环境', vps: '网络环境', cleaner: 'Claude 身份', browser: '浏览器环境', help: '使用说明' };
function App() {
    const {t}=useI18n();
    const [page, setPage] = useState<Page>('system'), [advancedBusy, setAdvancedBusy] = useState(false), [updateBusy, setUpdateBusy] = useState(false);
    const locked = advancedBusy || updateBusy;
    return <><div className="drag-strip" onMouseDown={e => { if (e.button === 0)
        void getCurrentWindow().startDragging(); }}/><div className="app-shell"><aside className="app-sidebar">
    <div className="app-brand"><strong>{t("RioBaga")}</strong></div>
    <nav aria-label={t("主要页面")}>{order.map(p => <Fragment key={p}>{p === 'help' && <hr className="nav-divider"/>}<button type="button" className="rb-nav" aria-current={page === p ? "page" : undefined} disabled={locked} onClick={() => setPage(p)}>{t(titles[p])}</button></Fragment>)}</nav>
    <div className="sidebar-bottom"><AppUpdate blocked={advancedBusy} onBusy={setUpdateBusy}/><LanguageSwitch /></div>
  </aside><main className="workspace" inert={updateBusy}>
    {page === 'system' && <div className="cleaner-slot"><SystemPanel onBusy={setAdvancedBusy}/></div>}{page === 'vps' && <div className="cleaner-slot"><VpsPanel/></div>}{page === 'cleaner' && <div className="cleaner-slot"><IdentityPanel onBusy={setAdvancedBusy}/></div>}{page === 'browser' && <div className="cleaner-slot"><BrowserPanel onBusy={setAdvancedBusy}/></div>}{page === 'help' && <div className="cleaner-slot"><HelpPanel/></div>}
  </main></div></>;
}
createRoot(document.getElementById('app')!).render(<MotionRoot><TooltipProvider delayDuration={600} skipDelayDuration={300}><App /></TooltipProvider></MotionRoot>);
