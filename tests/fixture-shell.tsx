/**
 * [INPUT]: 页面名与真实页面组件；样式来自 src/index.css + src/style.css（由各夹具在组件之前引入，与 main.tsx 的级联顺序相同）。
 * [OUTPUT]: 与 main.tsx 同结构的外壳：drag-strip、app-shell、app-sidebar（品牌/五项导航/版本/语言静态占位）、main.workspace > div.cleaner-slot；默认铺满视口（等同应用窗口），可用 ?w=&h= 固定像素尺寸。
 * [POS]: 仅供 tests/*.html 夹具使用，使夹具中的内距、滚动与宽度和真实应用一致；不调用任何 Tauri 命令。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import {Fragment, type ReactNode} from 'react';

const pages = [['system', '电脑环境'], ['vps', '网络环境'], ['cleaner', 'Claude 身份'], ['browser', '浏览器环境'], ['help', '使用说明']] as const;
export type FixturePage = (typeof pages)[number][0];

export function FixtureShell({page, children}: {page: FixturePage; children: ReactNode}) {
  const query = new URLSearchParams(location.search);
  const width = Number(query.get('w')) || '100vw', height = Number(query.get('h')) || '100vh';
  return <div data-fixture-shell style={{width, height, overflow: 'hidden'}}>
    <div className="drag-strip"/>
    <div className="app-shell">
      <aside className="app-sidebar">
        <div className="app-brand"><strong>RioBaga</strong></div>
        <nav aria-label="fixture-nav">{pages.map(([id, label]) => <Fragment key={id}>{id === 'help' && <hr className="nav-divider"/>}<button type="button" tabIndex={-1} className="rb-nav" aria-current={id === page ? 'page' : undefined}>{label}</button></Fragment>)}</nav>
        <div className="sidebar-bottom"><small className="app-version">v0.0.0-fixture</small></div>
      </aside>
      <main className="workspace"><div className="cleaner-slot">{children}</div></main>
    </div>
  </div>;
}
