/**
 * [INPUT]: 当前保留策略、锁定状态与策略变更回调；policyRows 七项定义来自 identity-model.ts。
 * [OUTPUT]: 紧贴卡片下方常驻的「切换账号时保持不变」七个切换片（aria-pressed=留在工作区：白底实线墨字绿点；跟身份走：淡底虚线弱化字空心点），桌面会话共享开启时会话历史强制保留、禁用外观并显示锁定原因（aria-describedby）。
 * [POS]: 身份页保留策略条；只修改页面策略状态，持久化由 identities.tsx 写入 ccbaga.identity-policy.v1。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import {useI18n} from './i18n';
import {policyRows, type IdentityPolicy} from './identity-model';

export type PolicyKey = (typeof policyRows)[number][0];

export function PolicyBar({policy, keeps, disabled, onChange}: {
  policy: IdentityPolicy; keeps: (key: PolicyKey) => boolean; disabled: boolean; onChange: (next: IdentityPolicy) => void;
}) {
  const {t} = useI18n();
  return <section className="rb-card rb-id-policy" aria-labelledby="identity-policy-title">
    <div className="rb-id-policy-head">
      <h2 id="identity-policy-title" className="rb-label">{t('切换账号时保持不变')}</h2>
      <p className="rb-muted">{t('白底实线、绿点的留在工作区；淡底虚线、空心点的跟着身份一起存走。登录信息始终跟身份走。')}</p>
    </div>
    <div className="rb-id-chips">
      {policyRows.map(([key, label]) => {
        const forced = key === 'history' && policy.desktop_sessions;
        const pressed = keeps(key);
        return <button type="button" key={key} className="rb-chip" aria-pressed={pressed} disabled={disabled || forced} data-forced={forced || undefined}
          title={forced ? t('会话历史已锁定为留在工作区：桌面会话共享开启时不能改。') : undefined} aria-describedby={forced ? 'identity-policy-lock' : undefined}
          onClick={() => onChange({...policy, [key]: !pressed})}>
          <span className="rb-id-chip-dot" aria-hidden="true"/>{t(label)}
        </button>;
      })}
    </div>
    {policy.desktop_sessions && <p id="identity-policy-lock" className="rb-muted rb-id-policy-lock">{t('会话历史已锁定为留在工作区：桌面会话共享开启时不能改。')}</p>}
  </section>;
}
