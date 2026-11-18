/**
 * [INPUT]: 页面拼好的各段内容；桌面会话共享段的策略、同步结果与回调。
 * [OUTPUT]: 「更多工具」分段（WAI-ARIA tablist，方向键/Home/End 切换，一次只显示一段，其他段保持挂载以保留进行中的状态与确认窗）；macOS 桌面 Code 会话共享段（默认关闭、开启强制保留会话历史、手动同步与数量结果）。
 * [POS]: 身份页工具区；不调用原生命令，段内功能由各子面板与 identities.tsx 负责。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import {useId, useRef, useState, type KeyboardEvent, type ReactNode} from 'react';
import {Checkbox} from 'cromma-ui';
import {useI18n} from './i18n';

export type ToolTab = {id: string; label: string; content: ReactNode};

export function ToolTabs({tabs}: {tabs: ToolTab[]}) {
  const {t} = useI18n();
  const base = useId();
  const [selected, setSelected] = useState(tabs[0]?.id ?? '');
  const buttons = useRef<Record<string, HTMLButtonElement | null>>({});
  const current = tabs.some(tab => tab.id === selected) ? selected : tabs[0]?.id;
  function key(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    const last = tabs.length - 1;
    const next = event.key === 'ArrowRight' ? (index === last ? 0 : index + 1)
      : event.key === 'ArrowLeft' ? (index === 0 ? last : index - 1)
      : event.key === 'Home' ? 0 : event.key === 'End' ? last : -1;
    if (next < 0) return;
    event.preventDefault();
    setSelected(tabs[next].id);
    buttons.current[tabs[next].id]?.focus();
  }
  return <section className="rb-card line rb-id-tools" aria-label={t('更多工具')}>
    <div className="rb-seg rb-id-tabs" role="tablist" aria-label={t('更多工具')}>
      {tabs.map((tab, index) => <button type="button" role="tab" key={tab.id} id={`${base}-${tab.id}-tab`}
        ref={node => { buttons.current[tab.id] = node; }}
        aria-selected={current === tab.id} aria-controls={`${base}-${tab.id}-panel`} tabIndex={current === tab.id ? 0 : -1}
        className={current === tab.id ? 'on' : undefined}
        onClick={() => setSelected(tab.id)} onKeyDown={event => key(event, index)}>{tab.label}</button>)}
    </div>
    {tabs.map(tab => <div role="tabpanel" key={tab.id} id={`${base}-${tab.id}-panel`} aria-labelledby={`${base}-${tab.id}-tab`}
      className="rb-id-tab-panel" hidden={current !== tab.id}>{tab.content}</div>)}
  </section>;
}

export function DesktopSharingPanel({enabled, locked, blocked, result, onToggle, onSync}: {
  enabled: boolean; locked: boolean; blocked: boolean;
  result: {accounts: number; added: number; repaired: number; skipped: number} | null;
  onToggle: (enabled: boolean) => void; onSync: () => void;
}) {
  const {t} = useI18n();
  return <div className="identity-confirm rb-id-sharing">
    <Checkbox label={t('跨账号共享桌面 Code 会话')} checked={enabled} disabled={locked} onCheckedChange={checked => onToggle(checked === true)}/>
    <p className="identity-caption">{t('默认关闭。开启后保留会话历史，切换时向本机已有账号目录补齐本地 Code 会话；继续对话使用当前账号。不会复制登录、定时任务或会话授权。')}</p>
    <p className="identity-caption">{t('仅支持 macOS 默认 Claude App 的本地会话。新账号登录一次后可手动同步；请先退出 Claude CLI/App，并在同步期间保持关闭。关闭共享只停止后续同步，已共享入口和正文保留。')}</p>
    <div><button type="button" className="rb-btn" disabled={blocked || !enabled} onClick={onSync}>{t('同步桌面会话')}</button></div>
    <p className="identity-caption">{t('切换时自动跟进可确认的最新聊天版本；不同分支需选择，旧正文保留。')}</p>
    {result && <p role="status">{t('桌面会话共享：{accounts} 个账号目录，新增 {added} 个入口，更新 {repaired} 个会话，跳过 {skipped} 条不支持的记录。', result)}</p>}
  </div>;
}
