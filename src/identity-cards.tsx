/**
 * [INPUT]: 页面传入的脱敏身份状态、当前时间、用量缓存与操作回调；Motion 弹簧、浏览器媒体订阅与 Radix DropdownMenu。
 * [OUTPUT]: 横向工牌轨道（原生滚动条/键盘 + 滚轮弹簧）、空身份/当前/已存三类竖式工牌，卡内脱敏邮箱/组织、订阅与本地凭据徽章、用量区，主操作直出，重命名·导出·删除收进右上角「···」菜单。
 * [POS]: 身份页的卡片区；只呈现，不调用原生命令，不接收令牌或凭据正文。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import {useEffect, useRef, useSyncExternalStore, type ReactNode} from 'react';
import {DropdownMenu} from 'radix-ui';
import {ShieldCheck} from 'cromma-ui/pika';
import {motion, useMotionValue, useSpring} from 'motion/react';
import {useI18n} from './i18n';
import {formatBytes} from './cleaner-model';
import {CardUsage} from './identity-usage';
import {credentialStatus, type CurrentIdentity, type Identity, type IdentityState, type IdentitySummary, type IdentityUsage} from './identity-model';

export function authLabel(status: string | null | undefined) {
  if (status === 'local_unexpired') return '本地未过期';
  if (status === 'refresh_available') return '可尝试续期';
  if (status === 'expired') return '本地已到期';
  if (status === 'api_key') return 'API 密钥';
  return '状态未知';
}

// ----- 原生媒体订阅：当前 Motion hook 只记录初始偏好，故动态切换由浏览器事件驱动 -----
const motionPreference = typeof window === 'undefined' ? null : window.matchMedia('(prefers-reduced-motion: reduce)');
const subscribeMotion = (notify: () => void) => {
  motionPreference?.addEventListener('change', notify);
  return () => motionPreference?.removeEventListener('change', notify);
};
export function useIdentityReducedMotion() {
  return useSyncExternalStore(subscribeMotion, () => motionPreference?.matches ?? true, () => true);
}

// ----- 用原生 scrollLeft 保留滚动条/键盘；Motion 仅提供滚轮弹簧 -----
export function IdentityRail({label, children}: {label: string; children: ReactNode}) {
  const rail = useRef<HTMLDivElement>(null);
  const reduceMotion = useIdentityReducedMotion();
  const scrollSpring = useSpring(0, {stiffness: 170, damping: 25, mass: .8});
  const scrollTarget = useRef(0);
  useEffect(() => {
    const element = rail.current;
    if (!element) return;
    const maximum = () => Math.max(0, element.scrollWidth - element.clientWidth);
    if (reduceMotion) {
      scrollTarget.current = element.scrollLeft;
      scrollSpring.jump(element.scrollLeft);
    }
    const unsubscribe = scrollSpring.on('change', value => {
      element.scrollLeft = Math.max(0, Math.min(maximum(), value));
    });
    function interrupt() {
      scrollTarget.current = element!.scrollLeft;
      scrollSpring.jump(element!.scrollLeft);
    }
    function scroll() {
      if (Math.abs(element!.scrollLeft - Math.max(0, Math.min(maximum(), scrollSpring.get()))) > 2) interrupt();
    }
    function wheel(event: WheelEvent) {
      if (event.ctrlKey || event.metaKey) return;
      const amount = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY;
      const delta = amount * (event.deltaMode === 1 ? 24 : event.deltaMode === 2 ? element!.clientWidth : 1);
      const next = Math.max(0, Math.min(maximum(), scrollTarget.current + delta));
      if (next === scrollTarget.current) return;
      event.preventDefault();
      scrollTarget.current = next;
      if (reduceMotion) scrollSpring.jump(next);
      else scrollSpring.set(next);
    }
    element.addEventListener('wheel', wheel, {passive: false});
    element.addEventListener('scroll', scroll, {passive: true});
    element.addEventListener('pointerdown', interrupt);
    return () => {
      unsubscribe();
      element.removeEventListener('wheel', wheel);
      element.removeEventListener('scroll', scroll);
      element.removeEventListener('pointerdown', interrupt);
    };
  }, [scrollSpring, reduceMotion]);
  return <div className="identity-card-rail" ref={rail} tabIndex={0} role="region" aria-label={label}>{children}</div>;
}

// ----- 工牌跟随指针微倾斜，离开后由弹簧回正；触摸与减弱动态保持静止 -----
function IdentityCard({className, children, label}: {className: string; children: ReactNode; label?: string}) {
  const reduced = useIdentityReducedMotion();
  const tiltX = useMotionValue(0), tiltY = useMotionValue(0), lift = useMotionValue(0);
  const rotateX = useSpring(tiltX, {stiffness: 240, damping: 22});
  const rotateY = useSpring(tiltY, {stiffness: 240, damping: 22});
  const y = useSpring(lift, {stiffness: 240, damping: 22});
  useEffect(() => {
    if (!reduced) return;
    tiltX.jump(0); tiltY.jump(0); lift.jump(0);
    rotateX.jump(0); rotateY.jump(0); y.jump(0);
  }, [reduced, tiltX, tiltY, lift, rotateX, rotateY, y]);
  const freeze = () => {
    rotateX.jump(rotateX.get());
    rotateY.jump(rotateY.get());
    y.jump(y.get());
  };
  return <motion.article className={className} aria-label={label} style={{rotateX, rotateY, y, transformPerspective: 850}}
    onPointerDown={freeze}
    onPointerMove={event => {
      if (reduced || event.pointerType !== 'mouse') return;
      if ((event.target as Element).closest('button')) { freeze(); return; }
      const box = event.currentTarget.getBoundingClientRect();
      tiltX.set(-(event.clientY - box.top - box.height / 2) / box.height * 9);
      tiltY.set((event.clientX - box.left - box.width / 2) / box.width * 9);
      lift.set(-4);
    }}
    onPointerLeave={() => { tiltX.set(0); tiltY.set(0); lift.set(0); }}>
    {children}
  </motion.article>;
}

// ----- 次要操作菜单：Radix 提供方向键/回车/Esc 与焦点归还；非模态避免与随后打开的确认窗争夺焦点锁 -----
type MenuItem = {key: string; label: string; aria?: string; danger?: boolean; onSelect: () => void};
function CardMenu({label, disabled, items}: {label: string; disabled: boolean; items: MenuItem[]}) {
  return <DropdownMenu.Root modal={false}>
    <DropdownMenu.Trigger asChild>
      <button type="button" className="rb-btn rb-id-menu-trigger" aria-label={label} disabled={disabled}><span aria-hidden="true">···</span></button>
    </DropdownMenu.Trigger>
    <DropdownMenu.Portal>
      <DropdownMenu.Content className="rb-id-menu" align="end" sideOffset={6}>
        {items.map(item => <DropdownMenu.Item key={item.key} className={`rb-id-menu-item${item.danger ? ' danger' : ''}`} aria-label={item.aria} onSelect={item.onSelect}>{item.label}</DropdownMenu.Item>)}
      </DropdownMenu.Content>
    </DropdownMenu.Portal>
  </DropdownMenu.Root>;
}

function AuthBadge({metadata, now}: {metadata: Pick<IdentitySummary, 'auth_status' | 'expires_at'> | null | undefined; now: number}) {
  const {t} = useI18n();
  const status = credentialStatus(metadata, now);
  return <span title={t('仅依据本地元数据，不代表服务端认证结果。')} className={`identity-status-badge auth-${status}`}>{t(authLabel(status))}</span>;
}

export function EmptyCard({selected, disabled, onUse}: {selected: boolean; disabled: boolean; onUse: () => void}) {
  const {t} = useI18n();
  return <IdentityCard className={`identity-card empty${selected ? ' selected' : ''}`} label={selected ? `${t('空身份')} · ${t('当前身份')}` : t('空身份')}>
    <div className="identity-card-head"><div className="identity-avatar" aria-hidden="true">∅</div></div>
    <h3>{t('空身份')}</h3>
    <p className="identity-empty-note">{t('清空本地登录身份；保留选定工作区资料，不影响服务端记录。')}</p>
    <div className="rb-id-actions"><button type="button" className="rb-btn" disabled={disabled} onClick={onUse}>{t('使用空身份')}</button></div>
  </IdentityCard>;
}

export function CurrentCard(props: {
  active: Identity | undefined; current: CurrentIdentity | undefined; name: string; state: IdentityState | null;
  loading: boolean; browserOnly: boolean; now: number; usage: IdentityUsage | undefined; usageLoading: boolean; disabled: boolean;
  onSave: () => void; onRename: (row: Identity) => void; onExport: (row: Identity) => void; onDelete: () => void;
}) {
  const {t} = useI18n();
  const {active, current, name, state, loading, browserOnly, now, disabled} = props;
  const processText = loading ? t('检查中')
    : state?.processes == null ? t('Claude 进程状态未知')
    : state.running ? t('Claude 客户端运行中') : t('Claude 客户端未运行');
  const saveLabel = browserOnly ? t(active ? '更新身份' : '保存身份') : active ? t('更新备份') : t('保存备份');
  const menu: MenuItem[] = [];
  if (active) {
    menu.push({key: 'rename', label: t('重命名'), onSelect: () => props.onRename(active)});
    menu.push({key: 'export', label: t('导出'), aria: t('导出保存备份'), onSelect: () => props.onExport(active)});
  }
  menu.push({key: 'delete', label: t('直接删除'), danger: true, onSelect: props.onDelete});
  return <IdentityCard className="identity-card current" label={active ? `${active.name} · ${t('当前身份')}` : t('当前身份')}>
    <div className="identity-card-head">
      <div className="identity-avatar" aria-hidden="true">{current?.display_name || current?.email ? Array.from(name)[0]?.toUpperCase() : <ShieldCheck size={20}/>}</div>
      <span className="identity-live-badge">{processText}</span>
    </div>
    <div className="rb-id-title"><h3>{name}</h3><span className="rb-id-current-pill">{t('当前')}</span></div>
    <div className="identity-account">
      {current?.email
        ? <span className="identity-email">{current.email}</span>
        : <span className="identity-account-unknown">{t(current?.unavailable ? 'CLI 账号信息暂不可读' : 'CLI 账号未识别')}</span>}
      {current?.organization && <span className="identity-organization">{current.organization}</span>}
    </div>
    <div className="identity-badges">
      <span className={`identity-status-badge ${active ? 'backed-up' : 'unbacked'}`}>{t(active ? '正在使用' : '未备份')}</span>
      {current?.plan && <span className="identity-status-badge plan">{current.plan}</span>}
      <AuthBadge metadata={current} now={now}/>
      {state?.browser.enabled && <span className="identity-browser-badge">{t('浏览器资料随身份切换')}</span>}
    </div>
    <CardUsage usage={props.usage} loading={props.usageLoading}/>
    <div className="rb-id-actions">
      <button type="button" className="rb-btn primary" disabled={disabled} onClick={props.onSave}>{saveLabel}</button>
      <CardMenu label={`${t('更多操作')} · ${name}`} disabled={disabled} items={menu}/>
    </div>
  </IdentityCard>;
}

export function SavedCard(props: {
  row: Identity; summary: IdentitySummary | undefined; bound: boolean; now: number; locale: string;
  usage: IdentityUsage | undefined; usageLoading: boolean; disabled: boolean;
  onSwitch: () => void; onRename: () => void; onExport: () => void; onDelete: () => void;
}) {
  const {t} = useI18n();
  const {row, summary, now, disabled} = props;
  const menu: MenuItem[] = [
    {key: 'rename', label: t('重命名'), onSelect: props.onRename},
    {key: 'export', label: t('导出'), aria: `${t('导出身份')} ${row.name}`, onSelect: props.onExport},
    {key: 'delete', label: t('删除'), danger: true, onSelect: props.onDelete},
  ];
  return <IdentityCard className="identity-card">
    <div className="identity-card-head">
      <div className="identity-avatar" aria-hidden="true">{Array.from(row.name)[0]?.toUpperCase()}</div>
      <span className="identity-size">{formatBytes(row.bytes)}</span>
    </div>
    <h3>{row.name}</h3>
    {summary && (summary.email || summary.organization) && <div className="identity-account saved-account">
      {summary.email && <span className="identity-email">{summary.email}</span>}
      {summary.organization && <span className="identity-organization">{summary.organization}</span>}
    </div>}
    <div className="identity-badges">
      {summary?.plan && <span className="identity-status-badge plan">{summary.plan}</span>}
      <AuthBadge metadata={summary} now={now}/>
      {props.bound && <span className="identity-browser-badge">{t('已关联浏览器资料')}</span>}
    </div>
    <CardUsage usage={props.usage} loading={props.usageLoading}/>
    <time>{new Date(row.saved_at).toLocaleString(props.locale === 'zh' ? 'zh-CN' : 'en-US', {month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit'})}</time>
    <div className="rb-id-actions">
      <button type="button" className="rb-btn" disabled={disabled} onClick={props.onSwitch}>{t('切换')}</button>
      <CardMenu label={`${t('更多操作')} · ${row.name}`} disabled={disabled} items={menu}/>
    </div>
  </IdentityCard>;
}
