/**
 * [INPUT]: 原生身份命令、保留策略持久化、共享进度、卡片区/策略条/分端登录区/工具分段/对话框集合子组件。
 * [OUTPUT]: 身份页主编排：页头（浏览器资料随身份切换开关、导入、唯一刷新含各账号用量）、恢复与浏览器重开横幅、工牌横排与卡片正下方真实阶段进度、常驻「切换账号时保持不变」策略条、「更多工具」分段（分端登录与更新/桌面会话共享/文件残留/CLI 管理）及全部确认窗。
 * [POS]: 身份管理 GUI；只呈现脱敏元数据，身份正文留在原生保险库；任务运行时经 onBusy 锁住导航；可选浏览器绑定/旧资料采用确认与重开重试。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import {useEffect, useState} from 'react';
import {Checkbox} from 'cromma-ui';
import {Refresh} from 'cromma-ui/pika';
import {invoke} from '@tauri-apps/api/core';
import {useI18n} from './i18n';
import {IdentityOperationProgress, useCleanupActivity} from './cleaner-activity';
import {IdentityTransfer} from './identity-transfer';
import {ClaudeCliPanel} from './claude-cli-panel';
import {ClaudeResiduePanel} from './claude-residue-panel';
import {CurrentCard, EmptyCard, IdentityRail, SavedCard} from './identity-cards';
import {PolicyBar, type PolicyKey} from './identity-policy-bar';
import {ClientSection} from './identity-clients';
import {DesktopSharingPanel, ToolTabs, type ToolTab} from './identity-tools';
import {AssociationDialog, BrowserLinkDialog, DeleteDialog, LoginDialog, RenameDialog, ReviewDialog, SessionConflictDialog, type LoginResult, type SessionChoice, type SessionConflict, type Target} from './identity-dialogs';
import {identityPolicyKey, readIdentityPolicy, policyRows, type Identity, type IdentityPolicy, type IdentityState, type IdentityUsage} from './identity-model';
import './identity.css';

// initialEmpty 已不再使用（傻瓜设置入口已移除）；保留可选入参以免合并前破坏调用方类型检查。
export function IdentityPanel({onBusy}: {onBusy?: (busy: boolean) => void; initialEmpty?: boolean}) {
  const {t, locale} = useI18n();
  const [state, setState] = useState<IdentityState | null>(null);
  const [now, setNow] = useState(Date.now);
  const [policy, setPolicy] = useState(readIdentityPolicy);
  const [loading, setLoading] = useState(true), [busy, setBusy] = useState(false), [cliBusy, setCliBusy] = useState(false);
  const [residueBusy, setResidueBusy] = useState(false), [residueRevision, setResidueRevision] = useState(0);
  const [error, setError] = useState('');
  const [diagnostics, setDiagnostics] = useState('');
  const [sharingResult, setSharingResult] = useState<{accounts: number; added: number; repaired: number; skipped: number; skipped_unsupported: number; skipped_missing_body: number; skipped_ambiguous_body: number; skipped_relation: number} | null>(null);
  const [sessionConflict, setSessionConflict] = useState<{conflict: SessionConflict; target: Target | 'sync'; choices: SessionChoice[]} | null>(null);
  const [selectedSession, setSelectedSession] = useState('');
  const [transfer, setTransfer] = useState<Identity | 'import' | null>(null), [transferBusy, setTransferBusy] = useState(false);
  const [browserReview, setBrowserReview] = useState<boolean | null>(null), [useExisting, setUseExisting] = useState(false);
  const [review, setReview] = useState<Target | null>(null);
  const [loginGuide, setLoginGuide] = useState(false);
  const [association, setAssociation] = useState<'cli' | 'desktop' | null>(null), [associationAccepted, setAssociationAccepted] = useState(false);
  const [loginEmail, setLoginEmail] = useState(''), [loginAccepted, setLoginAccepted] = useState(false), [loginBusy, setLoginBusy] = useState(false);
  const [loginResult, setLoginResult] = useState<LoginResult | null>(null);
  const [name, setName] = useState(t('当前身份'));
  const [rename, setRename] = useState<Identity | null>(null), [renameValue, setRenameValue] = useState('');
  const [deleteBrowser, setDeleteBrowser] = useState(false);
  const [deletion, setDeletion] = useState<Identity | 'current' | null>(null), [accepted, setAccepted] = useState(false);
  const activity = useCleanupActivity();
  const [usageBusy, setUsageBusy] = useState<Set<string>>(new Set()), [liveUsage, setLiveUsage] = useState<Record<string, IdentityUsage>>({});
  const locked = usageBusy.size > 0 || busy || !!sessionConflict || loginBusy || cliBusy || residueBusy || transferBusy || transfer !== null
    || loginGuide || !!association || !!review || !!rename || !!deletion || browserReview !== null;
  const current = state?.current;
  // 当前档案按 workspace_id 在原卡高亮，不追加重复卡。
  const active = state?.records.find(row => row.id === (state.workspace_id === undefined ? state.active : state.workspace_id));
  const associationTarget = state?.records.find(row => row.id === (state.browser.enabled ? state.browser.current_id : active?.id ?? state.last_selected));
  const browserOnly = !!state?.browser_profile_present && !current?.present;
  const currentName = active?.name || current?.display_name || current?.email?.split('@')[0] || t('当前身份');
  const saved = state?.records ?? [];
  const blocked = locked || loading || !state || !!state?.recovery;
  const keeps = (key: PolicyKey) => policy[key] || (key === 'history' && policy.desktop_sessions);
  const kept = policyRows.filter(([key]) => keeps(key)).map(([, label]) => t(label));
  const removed = policyRows.filter(([key]) => !keeps(key)).map(([, label]) => t(label));
  const emptySelected = current?.present === false && !state?.browser_profile_present && !active;

  useEffect(() => { void read(); }, []);
  useEffect(() => {
    const deadlines = [state?.current, ...Object.values(state?.summaries ?? {})]
      .map(item => item?.expires_at)
      .filter((expiry): expiry is number => typeof expiry === 'number' && Number.isSafeInteger(expiry) && expiry > now);
    if (!deadlines.length) return;
    const timer = setTimeout(() => setNow(Date.now()), Math.min(2147483647, Math.max(1, Math.min(...deadlines) - Date.now())));
    return () => clearTimeout(timer);
  }, [state, now]);
  useEffect(() => { onBusy?.(locked); return () => onBusy?.(false); }, [locked, onBusy]);

  // ----- 保留策略：页面状态与本机持久化同步写入（含恢复时采用旧档案策略） -----
  function updatePolicy(next: IdentityPolicy) {
    setPolicy(next);
    try { localStorage.setItem(identityPolicyKey, JSON.stringify(next)); }
    catch { setError('保留选项未能保存；重新打开页面后请再次核对。'); }
  }

  async function read(invalidateResidue = true) {
    setLoading(true);
    try {
      const next = await invoke<IdentityState>('identities_read');
      setState(next);
      if (invalidateResidue) setResidueRevision(value => value + 1);
      setError('');
      return true;
    } catch (e) { setError(String(e)); return false; }
    finally { setLoading(false); }
  }
  // 页面唯一的刷新：先读本地状态，再逐个读取每个账号的官方用量（当前账号不续期，已存账号由原生层代为续期）。
  async function refreshAll() {
    if (!await read()) return;
    const next = await invoke<IdentityState>('identities_read').catch(() => null);
    if (!next) return;
    const activeId = next.workspace_id === undefined ? next.active : next.workspace_id;
    const targets: [string, () => Promise<IdentityUsage>][] = [];
    if (next.current.present || next.current.email)
      targets.push([activeId && next.records.some(r => r.id === activeId) ? activeId : 'current', () => invoke<IdentityUsage>('identities_usage')]);
    for (const record of next.records)
      if (record.id !== activeId) targets.push([record.id, () => invoke<IdentityUsage>('identities_usage_saved', {id: record.id})]);
    setUsageBusy(new Set(targets.map(([key]) => key)));
    for (const [key, load] of targets) {
      try { const usage = await load(); setLiveUsage(value => ({...value, [key]: usage})); }
      catch (e) { setLiveUsage(value => ({...value, [key]: {status: 'failed', fetched_at: Date.now(), message: String(e)}})); }
      finally { setUsageBusy(value => { const rest = new Set(value); rest.delete(key); return rest; }); }
    }
  }
  async function refreshAfterError() {
    setResidueRevision(value => value + 1);
    try { setState(await invoke<IdentityState>('identities_read')); } catch { /* 保留操作错误，不掩盖恢复失败。 */ }
  }
  async function associate() {
    if (!association || !associationTarget || !associationAccepted || busy) return;
    setBusy(true); setError('');
    try {
      const next = await invoke<IdentityState>('identities_associate', {id: associationTarget.id, client: association, confirmed: true, onProgress: activity.start('identity', '更新指定端资料')});
      setState(next); setAssociation(null); setResidueRevision(value => value + 1);
      activity.finish(true, '指定端资料已更新到原档案；其他端、工作区与 B 绑定保持不变。');
    } catch (e) { setError(String(e)); activity.finish(false, String(e)); await refreshAfterError(); }
    finally { setBusy(false); setAssociationAccepted(false); }
  }
  async function login() {
    if (loginBusy || !loginAccepted || !loginEmail.trim()) return;
    setLoginBusy(true); setLoginResult(null); setError('');
    try {
      const result = await invoke<LoginResult>('identities_login', {email: loginEmail.trim(), onProgress: activity.start('login', 'CLI 官方登录')});
      setLoginResult(result);
      activity.finish(true, 'CLI 官方登录已完成；邮箱符合输入目标，B 组织未自动核对，尚未关联档案。');
      await read();
    } catch (e) { setError(String(e)); activity.finish(false, String(e)); await refreshAfterError(); }
    finally { setLoginBusy(false); setLoginAccepted(false); }
  }
  async function cancelLogin() {
    try { await invoke('identities_login_cancel'); } catch (e) { setError(String(e)); }
  }
  function closeLogin() { setLoginGuide(false); setLoginAccepted(false); setLoginResult(null); setError(''); }
  async function configureBrowser() {
    if (browserReview === null || busy) return;
    const enabled = browserReview;
    setBusy(true); setError('');
    try { setState(await invoke<IdentityState>('identities_browser', {enabled, useExisting})); setBrowserReview(null); }
    catch (e) { setError(String(e)); await refreshAfterError(); }
    finally { setBusy(false); }
  }
  async function retryBrowser() {
    if (busy) return;
    setBusy(true); setError('');
    try { await invoke('browser_launch', {onProgress: activity.start('browser')}); activity.finish(true, '浏览器资料已与身份同步。'); await read(); }
    catch (e) { setError(String(e)); activity.finish(false, String(e)); }
    finally { setBusy(false); }
  }
  function choose(target: Target) {
    setName(target === 'recovery' ? t('恢复前状态') : target === 'save' ? (active?.name || currentName).slice(0, 64) : currentName.slice(0, 64));
    if (target === 'recovery') {
      const previous = state?.records.find(row => row.id === state.recovery);
      if (previous) updatePolicy({...previous.policy});
    }
    setReview(target); setError('');
  }
  async function stop() {
    await invoke('identities_stop', {onProgress: activity.channel()});
  }
  function handleSessionConflict(error: unknown, target: Target | 'sync', choices: SessionChoice[]) {
    const message = String(error), prefix = 'DESKTOP_SESSION_CHOICE:', start = message.indexOf(prefix);
    if (start < 0) return false;
    try {
      const conflict = JSON.parse(message.slice(start + prefix.length)) as SessionConflict;
      if (!conflict.session || !conflict.revision || !Array.isArray(conflict.options) || conflict.options.length < 2) return false;
      setSessionConflict({conflict, target, choices: choices.filter(c => c.session !== conflict.session)});
      setSelectedSession(''); setError('');
      return true;
    } catch { return false; }
  }
  async function change(target: Target | null = review, sessionChoices: SessionChoice[] = []) {
    if (!target || busy) return;
    setReview(null); setSessionConflict(null); setBusy(true); setError('');
    const onProgress = activity.start('identity', target === 'recovery' ? '恢复原身份' : target === 'save' ? '保存当前身份备份' : target === 'empty' ? '切换到空身份' : '切换身份');
    try {
      await stop();
      const next = target === 'save'
        ? await invoke<IdentityState>('identities_save', {currentName: name.trim(), policy, onProgress})
        : await invoke<IdentityState>('identities_switch', {targetId: typeof target === 'object' ? target.id : 'empty', currentName: name.trim(), policy, recover: target === 'recovery', sessionChoices, onProgress});
      setState(next);
      setResidueRevision(value => value + 1);
      activity.finish(true, next.browser_warning || (target === 'recovery' ? '原身份已恢复。'
        : target === 'save' ? (browserOnly ? '当前身份已保存，浏览器资料仍保留在本机。' : '当前身份已加密备份，工作区保持不变。')
        : '身份切换完成。'));
    } catch (e) {
      const conflict = handleSessionConflict(e, target, sessionChoices);
      if (!conflict) setError(String(e));
      activity.finish(false, conflict ? '会话存在不同版本，请选择继续使用的版本；身份尚未切换。' : String(e));
      await refreshAfterError();
    } finally { setBusy(false); }
  }
  async function copyDiagnostics() {
    try { const lines = await invoke<number>('copy_diagnostics'); setDiagnostics(t('已复制 {lines} 行诊断日志，可直接粘贴发给开发者。', {lines})); }
    catch (e) { setDiagnostics(t(String(e))); }
  }
  function confirmDelete(target: Identity | 'current') { setDeletion(target); setAccepted(false); setDeleteBrowser(false); setError(''); }
  async function remove() {
    if (!deletion || !accepted || busy) return;
    const target = deletion;
    setDeletion(null); setBusy(true); setError('');
    const onProgress = activity.start('identity', target === 'current' ? '直接删除当前身份' : '删除暂存身份');
    try {
      if (target === 'current') await stop();
      const next = await invoke<IdentityState>('identities_delete', {id: target === 'current' ? 'current' : target.id, policy, deleteBrowser: target !== 'current' && deleteBrowser, onProgress});
      setState(next);
      setResidueRevision(value => value + 1);
      activity.finish(true, next.browser_warning || '所选本地身份已直接删除。');
    } catch (e) { setError(String(e)); activity.finish(false, String(e)); await refreshAfterError(); }
    finally { setBusy(false); }
  }
  function startRename(row: Identity) { setRename(row); setRenameValue(row.name); setError(''); }
  async function saveName() {
    if (!rename || busy) return;
    setBusy(true);
    try { setState(await invoke<IdentityState>('identities_rename', {id: rename.id, value: renameValue})); setRename(null); setError(''); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  async function syncDesktopSessions(sessionChoices: SessionChoice[] = []) {
    if (busy || !policy.desktop_sessions || state?.recovery) return;
    setSessionConflict(null); setBusy(true); setError(''); setSharingResult(null);
    try { setSharingResult(await invoke('identities_sync_desktop_sessions', {sessionChoices})); }
    catch (e) { if (!handleSessionConflict(e, 'sync', sessionChoices)) setError(String(e)); }
    finally { setBusy(false); }
  }
  function continueSession() {
    if (!sessionConflict || !selectedSession) return;
    const {conflict, target, choices} = sessionConflict;
    const next = [...choices, {session: conflict.session, revision: conflict.revision, cli_id: selectedSession}];
    if (target === 'sync') void syncDesktopSessions(next);
    else void change(target, next);
  }

  const currentCard = <CurrentCard key={active?.id ?? 'unsaved-current'} active={active} current={current} name={currentName} state={state}
    loading={loading} browserOnly={browserOnly} now={now} disabled={blocked}
    usage={liveUsage[active?.id ?? 'current'] ?? (active ? state?.usages?.[active.id] : undefined)} usageLoading={usageBusy.has(active?.id ?? 'current')}
    onSave={() => choose('save')} onRename={startRename} onExport={row => setTransfer(row)} onDelete={() => confirmDelete('current')}/>;

  const tools: ToolTab[] = [
    {id: 'clients', label: t('分端登录与更新'), content: <ClientSection current={current} target={associationTarget}
      summary={associationTarget ? state?.summaries[associationTarget.id] : undefined} now={now} disabled={blocked}
      onLogin={() => { setLoginGuide(true); void read(false); }}
      onAssociate={client => { setAssociation(client); setAssociationAccepted(false); setError(''); }}/>},
    ...(state?.desktop_sessions_supported ? [{id: 'sharing', label: t('桌面会话共享'), content: <DesktopSharingPanel enabled={policy.desktop_sessions}
      locked={locked} blocked={blocked} result={sharingResult}
      onToggle={enabled => { setSharingResult(null); updatePolicy({...policy, desktop_sessions: enabled, history: enabled ? true : policy.history}); }}
      onSync={() => void syncDesktopSessions()}/>}] : []),
    {id: 'residue', label: t('文件残留'), content: <ClaudeResiduePanel activity={activity} blocked={locked || loading || !state || !!state?.recovery}
      revision={residueRevision} onBusy={setResidueBusy}
      onChanged={async () => { if (!await read(false)) throw new Error('身份状态刷新失败，请重新检查。'); }}/>},
    {id: 'cli', label: t('CLI 管理'), content: <ClaudeCliPanel blocked={locked || loading || !!state?.recovery} onBusy={setCliBusy}/>},
  ];

  return <section className="identity-page rb-page rb-identity">
    <header className="rb-id-head">
      <h1 className="rb-h1">{t('Claude 身份管理')}</h1>
      <Checkbox label={t('浏览器资料随身份切换')} checked={state?.browser.enabled ?? false} disabled={blocked}
        onCheckedChange={value => { setUseExisting(false); setBrowserReview(value === true); setError(''); }}/>
      <button type="button" className="rb-link" disabled={locked || loading} onClick={() => void refreshAll()}>
        <Refresh aria-hidden size={14} className={loading || usageBusy.size > 0 ? 'spin' : ''}/>{t('刷新')}
      </button>
      <button type="button" className="rb-btn" disabled={blocked} onClick={() => setTransfer('import')}>{t('导入')}</button>
    </header>
    {error && <p role="alert" className="rb-id-banner rb-err">{t(error)}</p>}
    <p className="rb-muted rb-id-diagnostics">
      <button type="button" className="rb-link" onClick={() => void copyDiagnostics()}>{t('复制诊断日志')}</button>{' '}
      {diagnostics ? <span role="status">{diagnostics}</span> : t('遇到问题时复制后发给开发者。日志只存在本机、不上传，只含操作阶段、数量与错误，不含令牌、聊天正文或文件内容。')}
    </p>
    {state?.recovery && <div className="rb-id-banner warn" role="alert">
      <strong>{t('有一次身份切换尚未完成')}</strong>
      <button type="button" className="rb-btn primary" disabled={locked || loading} onClick={() => choose('recovery')}>{t('恢复原身份')}</button>
    </div>}
    {state?.browser_warning && <div className="rb-id-banner warn" role="alert">
      <span>{t(state.browser_warning)}</span>
      <button type="button" className="rb-btn" disabled={blocked} onClick={() => void retryBrowser()}>{t('打开浏览器')}</button>
    </div>}
    <div className="rb-id-library">
      <IdentityRail label={t('身份')}>
        <EmptyCard selected={emptySelected} disabled={blocked} onUse={() => choose('empty')}/>
        {(loading || current?.present || browserOnly) && !active && currentCard}
        {saved.map(row => row.id === active?.id ? currentCard : <SavedCard key={row.id} row={row} summary={state?.summaries?.[row.id]}
          bound={!!state?.browser.bound_ids.includes(row.id)} now={now} locale={locale} disabled={blocked}
          usage={liveUsage[row.id] ?? state?.usages?.[row.id]} usageLoading={usageBusy.has(row.id)}
          onSwitch={() => choose(row)} onRename={() => startRename(row)} onExport={() => setTransfer(row)} onDelete={() => confirmDelete(row)}/>)}
      </IdentityRail>
      <IdentityOperationProgress activity={activity.state}/>
    </div>
    <PolicyBar policy={policy} keeps={keeps} disabled={locked} onChange={updatePolicy}/>
    <ToolTabs tabs={tools}/>

    <IdentityTransfer target={transfer} hasBrowser={typeof transfer === 'object' && transfer !== null && !!state?.browser.bound_ids.includes(transfer.id)}
      onClose={() => setTransfer(null)} onBusy={setTransferBusy} onState={setState} activity={activity}/>
    <SessionConflictDialog value={sessionConflict} selected={selectedSession} busy={busy}
      onSelect={setSelectedSession} onCancel={() => setSessionConflict(null)} onContinue={continueSession}/>
    <ReviewDialog review={review} state={state} active={active} browserOnly={browserOnly} busy={busy} loading={loading}
      name={name} kept={kept} removed={removed} onName={setName} onCancel={() => setReview(null)} onConfirm={() => void change()}/>
    <DeleteDialog deletion={deletion} state={state} busy={busy} kept={kept} removed={removed} accepted={accepted} deleteBrowser={deleteBrowser}
      onAccepted={setAccepted} onDeleteBrowser={setDeleteBrowser} onCancel={() => setDeletion(null)} onConfirm={() => void remove()}/>
    <BrowserLinkDialog review={browserReview} busy={busy} useExisting={useExisting} error={error}
      onUseExisting={setUseExisting} onCancel={() => setBrowserReview(null)} onConfirm={() => void configureBrowser()}/>
    <AssociationDialog association={association} target={associationTarget} state={state} busy={busy} accepted={associationAccepted} error={error}
      onAccepted={setAssociationAccepted} onDismiss={() => { setAssociation(null); setAssociationAccepted(false); setError(''); }}
      onCancel={() => setAssociation(null)} onConfirm={() => void associate()}/>
    <LoginDialog open={loginGuide} state={state} loading={loading} busy={loginBusy} email={loginEmail} accepted={loginAccepted} result={loginResult} error={error}
      onOpenChange={open => { setLoginGuide(open); setLoginAccepted(false); setLoginResult(null); setError(''); }}
      onClose={closeLogin} onCancelLogin={() => void cancelLogin()} onRefresh={() => void read(false)}
      onEmail={value => { setLoginEmail(value); setLoginAccepted(false); setLoginResult(null); }} onAccepted={setLoginAccepted} onLogin={() => void login()}/>
    <RenameDialog target={rename} value={renameValue} busy={busy} error={error} onValue={setRenameValue}
      onCancel={() => { setRename(null); setError(''); }} onSave={() => void saveName()}/>
  </section>;
}
