/**
 * [INPUT]: 页面持有的对话框状态、脱敏身份状态、保留策略摘要与确认回调。
 * [OUTPUT]: 身份页全部确认窗：会话版本选择、切换/保存/恢复确认（目标档案资料预览、维护窗口与进程预览）、直接删除/删除暂存 Alert（勾选确认、默认聚焦取消）、浏览器联动、单端更新、CLI 官方登录与重命名。
 * [POS]: 身份页对话框集合；只呈现与收集确认，原生调用仍由 identities.tsx 执行，不接收令牌或凭据正文。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import {Button, Checkbox, Input} from 'cromma-ui';
import {useI18n} from './i18n';
import {AppDialog} from './dialog';
import type {ClientMaterials, Identity, IdentityState} from './identity-model';

export type Target = Identity | 'empty' | 'recovery' | 'save';
export type SessionChoice = {session: string; revision: string; cli_id: string};
export type SessionConflict = {session: string; title: string | null; revision: string; options: {cli_id: string; messages: number; last_message_at: string | null}[]};
export type LoginResult = {email: string; organization: string | null; subscription: string | null};

const dateLocale = (locale: string) => locale === 'zh' ? 'zh-CN' : 'en-US';

// ----- 目标档案内资料（确认窗只读预览）；旧档案未标注保持未知 -----
function SavedMaterialRows({clients, browser}: {clients?: ClientMaterials | null; browser?: boolean}) {
  const {t} = useI18n();
  const material = (value: string | undefined) => t(value === 'present' ? '有受支持资料' : value === 'absent' ? '未发现受支持资料' : '资料状态未知');
  return <div className="identity-confirm">
    <div className="identity-confirm-keep"><span>{t('Claude Code CLI')}</span><strong>{material(clients?.cli)}</strong></div>
    <div className="identity-confirm-keep"><span>{t('Claude App')}</span><strong>{material(clients?.desktop)}</strong></div>
    <div className="identity-confirm-keep"><span>CCBaga-B</span><strong>{t(browser === true ? '已关联本机浏览器资料' : browser === false ? '无本机浏览器资料关联' : '资料状态未知')}</strong></div>
  </div>;
}

function ProcessPreview({state, short = false}: {state: IdentityState | null; short?: boolean}) {
  const {t} = useI18n();
  if (state?.processes == null) return <p>{t('进程状态未知；执行前将重新核验。')}</p>;
  if (!state.processes.length) return <p>{t('未检测到 Claude 进程。')}</p>;
  const name = (entry: string) => !short ? entry : entry === 'Claude Code' ? 'CLI' : entry === 'Claude Desktop' ? 'App' : entry;
  const list = <ul>{state.processes.map(process => <li key={process.pid}><code>PID {process.pid}</code><span>{name(process.entrypoint)}</span></li>)}</ul>;
  return short ? <details><summary>{t('查看运行程序（{v0}）', {v0: state.processes.length})}</summary>{list}</details> : list;
}

export function SessionConflictDialog({value, selected, busy, onSelect, onCancel, onContinue}: {
  value: {conflict: SessionConflict} | null; selected: string; busy: boolean;
  onSelect: (id: string) => void; onCancel: () => void; onContinue: () => void;
}) {
  const {t, locale} = useI18n();
  return <AppDialog alert open={!!value} onOpenChange={open => { if (!open && !busy) onCancel(); }}
    title={t('选择继续聊天的版本')}
    description={t('这些版本无法自动合并。选择后各账号将使用该版本继续，其他版本的旧索引和正文仍保留。')}
    footer={<>
      <Button variant="secondary" data-dialog-cancel disabled={busy} onClick={onCancel}>{t('取消')}</Button>
      <Button variant="ink" disabled={busy || !selected} onClick={onContinue}>{t('使用所选版本继续')}</Button>
    </>}>
    <div className="identity-confirm">
      <strong>{value?.conflict.title || t('未命名会话')}</strong>
      {value?.conflict.options.map((option, index) => <label className="identity-confirm-keep" key={option.cli_id}>
        <input type="radio" name="session-version" value={option.cli_id} checked={selected === option.cli_id} onChange={() => onSelect(option.cli_id)}/>
        <span>
          {t('版本 {number}', {number: index + 1})} · {option.cli_id.slice(-8)}<br/>
          {t('{count} 条消息', {count: option.messages})} · {option.last_message_at ? new Date(option.last_message_at).toLocaleString(dateLocale(locale)) : t('时间未知')}
        </span>
      </label>)}
    </div>
  </AppDialog>;
}

export function ReviewDialog(props: {
  review: Target | null; state: IdentityState | null; active: Identity | undefined; browserOnly: boolean;
  busy: boolean; loading: boolean; name: string; kept: string[]; removed: string[];
  onName: (value: string) => void; onCancel: () => void; onConfirm: () => void;
}) {
  const {t} = useI18n();
  const {review, state, active, browserOnly} = props;
  const title = review === 'recovery' ? t('恢复原身份')
    : review === 'save' ? t(browserOnly ? '保存当前身份' : active ? '更新当前身份备份' : '保存当前身份备份')
    : review === 'empty' ? t('切换到空身份') : t('切换到已有身份');
  const description = t(review === 'save'
    ? (browserOnly ? '将保存当前浏览器资料与身份的关联；当前工作区和浏览器保持不变。' : '将结束已识别的 Claude 进程，把当前身份原地加密写入保险库；当前工作区与登录保持不变。')
    : '先压缩暂存当前 Claude Code 与 Desktop 状态，再切换。将结束已识别的 Claude 进程，未保存任务会中断。');
  const confirm = review === 'recovery' ? t('确认恢复') : review === 'save' ? t(browserOnly ? '确认保存身份' : '确认保存备份') : t('保存当前身份并切换');
  const target = typeof review === 'object' && review ? review : null;
  return <AppDialog open={!!review} onOpenChange={open => { if (!open && !props.busy) props.onCancel(); }} title={title} description={description}
    footer={<>
      <Button variant="secondary" onClick={props.onCancel}>{t('取消')}</Button>
      <Button variant="ink" disabled={props.busy || props.loading || !state || !props.name.trim() || (review !== 'recovery' && !!state.recovery)} onClick={props.onConfirm}>{confirm}</Button>
    </>}>
    <div className="identity-confirm">
      <p>{t('人工维护窗口：保存、切换或恢复期间，不要从 Dock、终端或其他入口启动、登录客户端或修改资料；等待操作结束。检查无法拦截所有外部写入。')}</p>
      {target && <>
        <strong>{t('目标档案内资料')}</strong>
        <SavedMaterialRows clients={state?.summaries[target.id]?.clients} browser={state?.browser.bound_ids.includes(target.id)}/>
        <p>{t('旧档案未标注时显示未知；网页资料仅绑定在本机，不在加密归档内。')}</p>
      </>}
      {active && review !== 'recovery'
        ? <div className="identity-confirm-keep"><span>{t('保存当前身份')}</span><strong>{active.name}</strong></div>
        : <>
          <label htmlFor="identity-current-name">{t('首次保存当前身份为')}</label>
          <Input id="identity-current-name" maxLength={64} value={props.name} onChange={e => props.onName(e.target.value)}/>
        </>}
      {target && <div className="identity-confirm-target"><span>{t('切换到')}</span><strong>{target.name}</strong></div>}
      {state?.browser_profile_present && <p>{t('浏览器资料保留在本机并关联此身份，不会打包进 Claude 加密归档；未识别到 CLI 账号不等于网页登录无效。')}</p>}
      {review === 'save' && !browserOnly && <div className="identity-confirm-keep"><span>{t('纳入加密备份')}</span><strong>{[t('登录与身份数据'), ...props.removed].join(' · ')}</strong></div>}
      <div className="identity-confirm-keep"><span>{t('留在工作区')}</span><strong>{props.kept.join(' · ') || t('无')}</strong></div>
      <div className="identity-active-processes">
        <span>{t('Claude 进程（确认后结束）')}</span>
        <ProcessPreview state={state}/>
        {state?.processes_truncated && <p>{t('仅显示前 32 个进程；执行时重新核验完整范围。')}</p>}
      </div>
      {state?.browser.enabled && review !== 'save' && <p>{t('将关闭并重开关联浏览器，请先保存网页任务。')}</p>}
      <p>{t('只管理本地状态；失效的登录可能需要重新认证。')}</p>
    </div>
  </AppDialog>;
}

export function DeleteDialog(props: {
  deletion: Identity | 'current' | null; state: IdentityState | null; busy: boolean; kept: string[]; removed: string[];
  accepted: boolean; deleteBrowser: boolean;
  onAccepted: (value: boolean) => void; onDeleteBrowser: (value: boolean) => void; onCancel: () => void; onConfirm: () => void;
}) {
  const {t} = useI18n();
  const {deletion, state} = props;
  const current = deletion === 'current';
  return <AppDialog alert className="identity-delete-alert" open={!!deletion} onOpenChange={open => { if (!open && !props.busy) props.onCancel(); }}
    title={current ? t('直接删除当前身份') : t('删除暂存身份')}
    description={current ? t('将结束 Claude 进程并删除所选本地数据。不会新增备份，已有暂存身份不受影响。') : t('永久删除这份暂存文件。当前工作区不受影响。')}
    footer={<>
      <Button variant="secondary" data-dialog-cancel onClick={props.onCancel}>{t('取消')}</Button>
      <Button className="identity-delete-button" variant="ink" disabled={props.busy || !props.accepted} onClick={props.onConfirm}>{t('确认直接删除')}</Button>
    </>}>
    <div className="identity-confirm">
      <div className="identity-delete-warning"><span aria-hidden="true">!</span><strong>{t('警告：此操作不可撤销，不会创建新备份。')}</strong></div>
      {current ? <>
        <div className="identity-confirm-keep"><span>{t('删除范围')}</span><strong>{[t('登录与身份数据'), ...props.removed].join(' · ')}</strong></div>
        <div className="identity-confirm-keep"><span>{t('留在工作区')}</span><strong>{props.kept.join(' · ') || t('无')}</strong></div>
      </> : <strong>{deletion?.name}</strong>}
      {typeof deletion === 'object' && deletion && state?.browser.bound_ids.includes(deletion.id)
        && <Checkbox label={t('同时永久删除关联浏览器资料')} checked={props.deleteBrowser} onCheckedChange={value => props.onDeleteBrowser(value === true)}/>}
      <Checkbox checked={props.accepted} onCheckedChange={value => props.onAccepted(value === true)} label={t('我知道此操作无法撤销')}/>
    </div>
  </AppDialog>;
}

export function BrowserLinkDialog({review, busy, useExisting, error, onUseExisting, onCancel, onConfirm}: {
  review: boolean | null; busy: boolean; useExisting: boolean; error: string;
  onUseExisting: (value: boolean) => void; onCancel: () => void; onConfirm: () => void;
}) {
  const {t} = useI18n();
  return <AppDialog open={review !== null} onOpenChange={open => { if (!open && !busy) onCancel(); }}
    title={t(review ? '联动浏览器' : '关闭浏览器联动')} description={t('将正常关闭并重开浏览器，请先保存网页任务。')}
    footer={<>
      <Button variant="secondary" disabled={busy} onClick={onCancel}>{t('取消')}</Button>
      <Button variant="ink" disabled={busy} loading={busy} onClick={onConfirm}>{t('确认')}</Button>
    </>}>
    <div className="identity-confirm">
      <p>{t(review ? '每个身份使用独立网页资料和稳定指纹，切换时自动恢复。首次仍需登录网站。' : '恢复使用原有通用浏览器资料。各身份的独立资料会保留。')}</p>
      {review && <Checkbox label={t('将现有浏览器资料关联给当前身份')} checked={useExisting} disabled={busy} onCheckedChange={value => onUseExisting(value === true)}/>}
      <p>{t('默认创建新资料，现有资料不会删除。资料隔离不会改变出口 IP。')}</p>
      {error && <p role="alert">{t(error)}</p>}
    </div>
  </AppDialog>;
}

export function AssociationDialog(props: {
  association: 'cli' | 'desktop' | null; target: Identity | undefined; state: IdentityState | null; busy: boolean;
  accepted: boolean; error: string; onAccepted: (value: boolean) => void; onDismiss: () => void; onCancel: () => void; onConfirm: () => void;
}) {
  const {t} = useI18n();
  const {association, target, state, busy} = props;
  return <AppDialog alert open={!!association} onOpenChange={open => { if (!open && !busy) props.onDismiss(); }} title={t('更新指定端资料')}
    footer={<>
      <Button variant="secondary" data-dialog-cancel disabled={busy} onClick={props.onCancel}>{t('取消')}</Button>
      <Button variant="ink" disabled={busy || !props.accepted || !target} onClick={props.onConfirm}>{t('确认更新该端')}</Button>
    </>}>
    <div className="identity-confirm">
      <strong>{target?.name} · {association === 'cli' ? 'CLI' : 'App'}</strong>
      <p>{t('只更新指定端的受支持资料，沿用目标档案保留策略；其他端使用原归档，不吸收其当前资料，不更改 B 绑定或切换工作区。')}</p>
      <p>{t('CLI 账号或组织不符会拒绝。App 目前只记录用户核对，不能据此宣称自动账号匹配或服务端登录有效。')}</p>
      <p>{t('请先自行退出 CLI/App。本操作不会结束程序；B 仅保存绑定，可保持运行。操作期间不要启动或登录客户端。')}</p>
      {target && <p>{t('目标 CLI 元数据')}：{state?.summaries[target.id]?.email || t('CLI 账号未识别')} · {state?.summaries[target.id]?.organization || t('组织未知')}</p>}
      <Checkbox label={t('我已核对该端账号与组织，已退出 CLI/App 并保持维护窗口')} checked={props.accepted} disabled={busy} onCheckedChange={value => props.onAccepted(value === true)}/>
      {props.error && <p role="alert">{t(props.error)}</p>}
    </div>
  </AppDialog>;
}

export function LoginDialog(props: {
  open: boolean; state: IdentityState | null; loading: boolean; busy: boolean; email: string; accepted: boolean;
  result: LoginResult | null; error: string;
  onOpenChange: (open: boolean) => void; onClose: () => void; onCancelLogin: () => void; onRefresh: () => void;
  onEmail: (value: string) => void; onAccepted: (value: boolean) => void; onLogin: () => void;
}) {
  const {t} = useI18n();
  const {state, busy, result} = props;
  return <AppDialog open={props.open} onOpenChange={open => { if (!busy) props.onOpenChange(open); }} title={t('登录与关联说明')}
    footer={<>
      {busy
        ? <Button variant="secondary" onClick={props.onCancelLogin}>{t('取消登录')}</Button>
        : <Button variant="secondary" onClick={props.onClose}>{t('关闭')}</Button>}
      <Button variant="ink" disabled={busy || !props.accepted || !props.email.trim()} onClick={props.onLogin}>{t('在当前 B 中授权 CLI')}</Button>
    </>}>
    <div className="identity-confirm">
      <p>{t('先切换到目标本地档案，再分别在官方客户端核对账号和组织。异号时不要更新备份；不要把卡片名称当作登录证明。')}</p>
      <p>{t('CLI 订阅登录需要 Pro、Max，或具有 CLI 权限的 Team / Enterprise。免费账号不适用；此入口不支持 Console / API 登录。')}</p>
      <div className="identity-active-processes">
        <span>{t('登录前检查')}</span>
        <p>{t('需要新登录时，请先保存任务并退出 CLI 和 App；B 浏览器保持打开。不会自动结束程序。已有同邮箱登录只核对状态。')}</p>
        {props.loading ? <p>{t('检查中')}</p> : <ProcessPreview state={state} short/>}
        {state?.processes_truncated && <p>{t('仅显示前 32 个进程；执行时重新核验完整范围。')}</p>}
        <Button variant="secondary" size="small" disabled={busy || props.loading} onClick={props.onRefresh}>{t('刷新进程')}</Button>
      </div>
      <label htmlFor="identity-login-email">{t('目标账号邮箱')}</label>
      <Input id="identity-login-email" aria-label={t('目标账号邮箱')} value={props.email} disabled={busy} maxLength={254} onChange={e => props.onEmail(e.target.value)}/>
      <Checkbox label={t('我已核对当前 B 资料与目标邮箱，允许官方 CLI 写入登录凭据')} checked={props.accepted} disabled={busy} onCheckedChange={value => props.onAccepted(value === true)}/>
      <p>{t('登录后请保存备份。取消不一定撤销已完成的登录。')}</p>
      {busy && <p role="status">{t('请在 B 中完成官方验证；邮箱与组织需要本人核对。')}</p>}
      {result && <div role="status">
        <strong>{t('CLI 官方回报')}</strong>
        <p>{result.email} · {result.organization || t('组织未知')} · {result.subscription || t('状态未知')}</p>
        <p>{t('邮箱符合输入目标；B 组织未自动核对，尚未关联档案。')}</p>
      </div>}
      {props.error && <p role="alert">{t(props.error)}</p>}
      <details>
        <summary>{t('支持范围与登录说明')}</summary>
        <p>{t('CLI：仅支持已验证的 macOS 原生 2.1.307。先打开目标 B 资料；官方授权页将在该资料的新标签中打开。已有登录只核对，不自动登出。')}</p>
        <p>{t('只完成官方登录，不保存或切换档案，不发送模型请求。最多等待 10 分钟；取消可能发生在凭据写入后，不自动登出或回滚。')}</p>
        <p>{t('App：使用官方登录入口，由本人完成验证；若跳转浏览器，先确认是目标 B 资料。B：沿用已绑定资料登录，不复制其他端令牌。')}</p>
      </details>
    </div>
  </AppDialog>;
}

export function RenameDialog({target, value, busy, error, onValue, onCancel, onSave}: {
  target: Identity | null; value: string; busy: boolean; error: string;
  onValue: (value: string) => void; onCancel: () => void; onSave: () => void;
}) {
  const {t} = useI18n();
  return <AppDialog open={!!target} onOpenChange={open => { if (!open && !busy) onCancel(); }} title={t('重命名身份')}
    footer={<>
      <Button variant="secondary" onClick={onCancel}>{t('取消')}</Button>
      <Button variant="ink" disabled={busy || !value.trim()} onClick={onSave}>{t('保存')}</Button>
    </>}>
    <Input aria-label={t('身份名称')} maxLength={64} value={value} onChange={e => onValue(e.target.value)}/>
    {error && <p role="alert">{t(error)}</p>}
  </AppDialog>;
}
