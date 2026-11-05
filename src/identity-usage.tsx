/**
 * [INPUT]: 单个身份的用量缓存/本次读取结果与读取中状态。
 * [OUTPUT]: 工牌内的紧凑用量区：会话（5 小时）与本周两条细进度、重置时间或「已重置」、异常时一行原因。
 * [POS]: 身份卡片的只读账号信息；联网读取由页面唯一的「刷新」统一触发（identities.tsx），本组件不发请求、不接收令牌。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import {useI18n} from './i18n';
import type {IdentityUsage, UsageWindow} from './identity-model';

const statusText:Record<Exclude<IdentityUsage['status'],'ok'>,string>={
 expired:'令牌已到期，运行一次 Claude Code 后再刷新',
 no_token:'没有可用的 CLI 或 Desktop 登录',
 rejected:'令牌被拒，需重新登录',
 failed:'用量读取失败',
 active:'正在使用',
 relogin:'续期失效，需重新登录',
 rotation_unsaved:'续期未保存，可能需重新登录',
};
const severe=new Set<IdentityUsage['status']>(['rejected','relogin','rotation_unsaved']);

/** A window whose reset time has passed no longer reflects the last read percentage. */
export function windowReset(value:UsageWindow|null|undefined, now:number) {
 const at=value?.resets_at?Date.parse(value.resets_at):NaN;
 return Number.isFinite(at)&&at<=now;
}
function resetText(value:UsageWindow|null|undefined, now:number, locale:string):{key:string;values:Record<string,string>}|null {
 const at=value?.resets_at?Date.parse(value.resets_at):NaN;
 if(!Number.isFinite(at))return null;
 const minutes=Math.ceil((at-now)/60e3);
 if(minutes>0&&minutes<24*60){
  const h=Math.floor(minutes/60), m=minutes%60;
  if(h===0)return {key:'{v0} 分后重置',values:{v0:String(m)}};
  return m===0?{key:'{v0} 小时后重置',values:{v0:String(h)}}:{key:'{v0} 小时 {v1} 分后重置',values:{v0:String(h),v1:String(m)}};
 }
 return {key:'{v0} 重置',values:{v0:new Date(at).toLocaleString(locale==='zh'?'zh-CN':'en-US',{month:'numeric',day:'numeric',hour:'2-digit',minute:'2-digit'})}};
}

function Meter({label, value, now}:{label:string; value:UsageWindow|null|undefined; now:number}) {
 const {t, locale}=useI18n();
 const reset=windowReset(value, now);
 const percent=value?.percent??0;
 const shown=value&&!reset?percent:0;
 const hint=reset?null:resetText(value, now, locale);
 return <div className={`card-usage-meter${reset?' reset':''}`}>
  <div className="card-usage-line"><span>{t(label)}{hint&&<small>{t(hint.key,hint.values)}</small>}</span><strong className={reset?'':percent>=90?'high':percent>=70?'mid':''}>{reset?t('已重置'):value?`${percent}%`:'—'}</strong></div>
  <div className="card-usage-track" role="progressbar" aria-label={t(label)} aria-valuemin={0} aria-valuemax={100} aria-valuenow={value&&!reset?percent:undefined}><span style={{width:`${shown}%`}}/></div>
 </div>;
}

export function CardUsage({usage, loading}:{usage:IdentityUsage|undefined; loading:boolean}) {
 const {t}=useI18n();
 const now=Date.now();
 if(loading&&!usage)return <div className="card-usage pending" aria-busy="true"><span>{t('读取用量中…')}</span></div>;
 if(!usage)return <div className="card-usage pending"><span>{t('用量未读取')}</span></div>;
 if(usage.status!=='ok')return <div className={`card-usage note${severe.has(usage.status)?' error':''}`} aria-busy={loading} title={usage.message?t(usage.message):undefined}><span>{t(statusText[usage.status])}</span></div>;
 return <div className={`card-usage${loading?' loading':''}`} aria-busy={loading} title={usage.source?t(usage.source==='desktop'?'用量来自 Desktop 登录':'用量来自 CLI 登录'):undefined}>
  <Meter label="会话" value={usage.session} now={now}/>
  <Meter label="本周" value={usage.week} now={now}/>
 </div>;
}
