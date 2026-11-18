/**
 * [INPUT]: 保存身份/浏览器绑定、原生一次性文件选择票据、密码、可信来源确认与共享真实进度流。
 * [OUTPUT]: 受控的加密迁移导出/导入确认；每次文件重选先清票据与确认，路径留在原生层，密码不持久化；导出默认文件名带档案名与时间，导出失败保留所选位置。
 * [POS]: 身份管理的迁移边界；仅处理保存档案，导入不切换身份，浏览器只可携带指纹配置。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import {useEffect,useState} from 'react';
import {Button,Checkbox,Input} from 'cromma-ui';
import {invoke} from '@tauri-apps/api/core';
import {AppDialog} from './dialog';
import {useI18n} from './i18n';
import {policyRows,type Identity,type IdentityState} from './identity-model';
import type {ActivityController} from './cleaner-activity';
type Grant={token:string;file_name:string};
const stampNow=()=>{const d=new Date(),p=(n:number)=>String(n).padStart(2,'0');return `${d.getFullYear()}${p(d.getMonth()+1)}${p(d.getDate())}-${p(d.getHours())}${p(d.getMinutes())}`;};
export function IdentityTransfer({target,hasBrowser,onClose,onBusy,onState,activity}:{target:Identity|'import'|null;hasBrowser:boolean;onClose:()=>void;onBusy:(busy:boolean)=>void;onState:(state:IdentityState)=>void;activity:ActivityController}){
 const {t,locale}=useI18n();
 const [grant,setGrant]=useState<Grant|null>(null),[password,setPassword]=useState(''),[repeat,setRepeat]=useState('');
 const [includeBrowser,setIncludeBrowser]=useState(false),[accepted,setAccepted]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState('');
 const exporting=typeof target==='object'&&target!==null;
 useEffect(()=>{setGrant(null);setPassword('');setRepeat('');setIncludeBrowser(false);setAccepted(false);setError('');},[target]);
 useEffect(()=>{onBusy(busy);return()=>onBusy(false);},[busy,onBusy]);
 const tooLong=new TextEncoder().encode(password).length>1024;
 const mismatch=exporting&&repeat.length>0&&password!==repeat;
 const valid=Array.from(password).length>=12&&!tooLong&&(!exporting||password===repeat);
 async function pick(){
  if(!target||busy)return;setGrant(null);setAccepted(false);setBusy(true);setError('');
  try{const chosen=await invoke<Grant|null>('identities_transfer_pick',exporting?{kind:'export',suggested:`${target.name} ${stampNow()}`}:{kind:'import'});if(chosen)setGrant(chosen);}
  catch(error){setError(String(error));}finally{setBusy(false);}
 }
 async function submit(){
  if(!target||!grant||!valid||!accepted||busy)return;
  const selected=grant;setGrant(null);setBusy(true);setError('');
  const secret=password;setPassword('');setRepeat('');
  const onProgress=activity.start('identity',exporting?'导出身份':'导入身份');
  try{
   const next=await invoke<IdentityState>(exporting?'identities_export':'identities_import',exporting?{id:target.id,grant:selected.token,password:secret,includeBrowser:includeBrowser&&hasBrowser,onProgress}:{grant:selected.token,password:secret,onProgress});
   onState(next);activity.finish(true,exporting?'身份已加密导出。':'身份已导入，当前身份保持不变。');onClose();
  }catch(error){setError(String(error));activity.finish(false,String(error));if(exporting)setGrant(selected);/* 原生层在文件未落盘时归还同一张票据，重试不必重选位置。 */try{onState(await invoke<IdentityState>('identities_read'));}catch{/* 保留原始迁移错误；刷新失败不声称回滚。 */}}
  finally{setBusy(false);}
 }
 return <AppDialog open={target!==null} onOpenChange={open=>{if(!open&&!busy)onClose();}} title={t(exporting?'导出身份':'导入身份')} description={t(exporting?'导出保存的登录与身份数据，请妥善保管文件和密码。':'仅支持相同操作系统；将新增保存身份，不切换当前身份。')} footer={<><Button variant="secondary" disabled={busy} onClick={onClose}>{t('取消')}</Button><Button variant="ink" disabled={busy||!grant||!valid||!accepted} loading={busy} onClick={()=>void submit()}>{t(exporting?'加密导出':'解密导入')}</Button></>}>
  <div className="identity-confirm identity-transfer">
   {exporting&&<div className="identity-confirm-target"><span>{t('身份')}</span><strong>{target.name}</strong></div>}
   {exporting&&<div className="identity-confirm-target"><span>{t('备份时间')}</span><time>{new Date(target.saved_at).toLocaleString(locale==='zh'?'zh-CN':'en-US')}</time></div>}
   <div className="identity-transfer-file"><span>{grant?.file_name||t('尚未选择文件')}</span><Button variant="secondary" size="small" disabled={busy} onClick={()=>void pick()}>{t(exporting?'选择保存位置':'选择文件')}</Button></div>
   {exporting&&<div className="identity-confirm-keep"><span>{t('纳入加密备份')}</span><strong>{[t('登录与身份数据'),...policyRows.filter(([key])=>!target.policy[key]).map(([,label])=>t(label))].join(' · ')}</strong></div>}
   <label htmlFor="identity-transfer-password">{t('迁移密码（至少 12 个字符）')}</label><Input id="identity-transfer-password" type="password" autoComplete="off" aria-invalid={tooLong} value={password} disabled={busy} maxLength={1024} onChange={event=>setPassword(event.target.value)}/>
   {exporting&&<><label htmlFor="identity-transfer-repeat">{t('确认迁移密码')}</label><Input id="identity-transfer-repeat" type="password" autoComplete="off" aria-invalid={mismatch} value={repeat} disabled={busy} maxLength={1024} onChange={event=>setRepeat(event.target.value)}/>{hasBrowser?<Checkbox checked={includeBrowser} disabled={busy} onCheckedChange={setIncludeBrowser} label={t('包含浏览器指纹配置（不含网页登录资料）')}/>:<p>{t('此备份未关联浏览器配置。')}</p>}</>}
   {(tooLong||mismatch)&&<p role="alert">{t(tooLong?'密码超过 1024 个 UTF-8 字节。':'两次密码不一致。')}</p>}
   <p>{t('不迁移浏览器 Cookie、密码或保留在工作区的资料。已有同账号或指纹配置时拒绝覆盖。')}</p>
   <Checkbox checked={accepted} disabled={busy} onCheckedChange={setAccepted} label={t(exporting?'我会妥善保管迁移文件和密码':'我确认备份来源可信，并了解恢复配置或插件可能执行其命令。')}/>
   {error&&<p role="alert" className="cleanup-feedback error">{t(error)}</p>}
  </div>
 </AppDialog>;
}
