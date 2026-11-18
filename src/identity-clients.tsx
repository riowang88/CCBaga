/**
 * [INPUT]: 当前脱敏身份、单端更新目标档案的核对来源、锁定状态与登录/单端更新回调。
 * [OUTPUT]: 「分端登录与更新」段：CLI 与 App 两行（账号元数据/本地凭据状态/核对来源）、登录 CLI 入口、只更新 CLI/App 三级操作与单端更新目标说明。
 * [POS]: 身份页分端登录区；只呈现本地元数据，不把资料存在或卡片名称当作登录证明，不调用原生命令。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import {useI18n} from './i18n';
import {authLabel} from './identity-cards';
import {credentialStatus, type CurrentIdentity, type Identity, type IdentitySummary} from './identity-model';

export function ClientSection({current, target, summary, now, disabled, onLogin, onAssociate}: {
  current: CurrentIdentity | undefined; target: Identity | undefined; summary: IdentitySummary | undefined; now: number; disabled: boolean;
  onLogin: () => void; onAssociate: (client: 'cli' | 'desktop') => void;
}) {
  const {t, locale} = useI18n();
  const checks = summary?.client_checks ?? {};
  const source = (key: 'cli' | 'desktop') => {
    const check = checks[key];
    if (!check) return null;
    return <span className="rb-id-client-source">
      {t(check.source === 'local_metadata' ? '来源：本地账号元数据' : '来源：用户核对，非自动匹配')} · {new Date(check.checked_at).toLocaleString(locale === 'zh' ? 'zh-CN' : 'en-US')}
    </span>;
  };
  return <div className="rb-id-clients" aria-label={t('分端资料与登录')} role="group">
    <p className="rb-muted">{t('桌面版请在 Claude App 中登录。各端需要分别登录同一个账号。')}</p>
    <div className="rb-row rb-id-client">
      <span className="rb-id-client-name">{t('Claude Code CLI')}</span>
      <span className="rb-muted rb-id-client-meta">
        <span>{current?.email || t(current?.unavailable ? 'CLI 账号信息暂不可读' : 'CLI 账号未识别')} · {t(authLabel(credentialStatus(current, now)))}</span>
        {source('cli')}
      </span>
      <button type="button" className="rb-link" disabled={disabled || !target} onClick={() => onAssociate('cli')}>{t('只更新 CLI')}</button>
      <button type="button" className="rb-btn" disabled={disabled} onClick={onLogin}>{t('登录与关联说明')}</button>
    </div>
    <div className="rb-row rb-id-client">
      <span className="rb-id-client-name">{t('Claude App')}</span>
      <span className="rb-muted rb-id-client-meta">
        <span>{t('资料存在不代表已登录；App 与 B 的账号及组织仍需在客户端确认。')}</span>
        {source('desktop')}
      </span>
      <button type="button" className="rb-link" disabled={disabled || !target} onClick={() => onAssociate('desktop')}>{t('只更新 App')}</button>
    </div>
    <p className="rb-row rb-muted">{t('单端更新目标')}：<strong>{target?.name || t('请先保存并选择目标本地档案')}</strong></p>
    <p className="rb-muted">{t('重新登录后，保存对应的备份。以后切换账号时会恢复这份备份。')}</p>
  </div>;
}
