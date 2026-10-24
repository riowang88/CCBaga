/**
 * [INPUT]: 回环 DNS 接口与当前 B 浏览器的图片请求。
 * [OUTPUT]: 本轮真实 DNS 解析器观测；空值保持未知。
 * [POS]: B 触发解析，A 只读取结果，不代测浏览器 DNS。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
export type DnsResolver={ip:string;country_code:string;country:string;city:string;isp:string;geo_error:string|null};
export type DnsObservation={id:string;resolvers:DnsResolver[];observed_at:number};
export type DnsDependencies={signal:AbortSignal;request:(path:string,signal:AbortSignal)=>Promise<unknown>;trigger:(id:string,index:number,signal:AbortSignal)=>Promise<void>;wait:(ms:number,signal:AbortSignal)=>Promise<void>};
export async function probeDns({signal,request,trigger,wait}:DnsDependencies):Promise<DnsObservation> {
 signal.throwIfAborted();
 const started=await request('dns/start',signal) as {id?:unknown};
 if(typeof started?.id!=='string'||!/^[a-f0-9]{48}$/.test(started.id))throw new Error('DNS 检测轮次无效。');
 const id=started.id;
 for(let i=1;i<=5;i++){
  signal.throwIfAborted();await trigger(id,i,signal);
  if(i<5)await wait(600,signal);
 }
 await wait(2000,signal);
 for(let attempt=0;attempt<3;attempt++){
  signal.throwIfAborted();
  const value=await request('dns/result/'+id,signal) as DnsObservation;
  signal.throwIfAborted();
  if(value?.id!==id)throw new Error('DNS 检测轮次不匹配。');
  if(!Array.isArray(value.resolvers)||value.resolvers.length>16||!Number.isFinite(value.observed_at)||value.resolvers.some(r=>!r||typeof r.ip!=='string'||typeof r.country_code!=='string'||typeof r.country!=='string'||typeof r.city!=='string'||typeof r.isp!=='string'||(r.geo_error!==null&&typeof r.geo_error!=='string')))throw new Error('DNS 服务返回无效的解析器列表。');
  if(value.resolvers.length||attempt===2)return value;
  await wait(2000,signal);
 }
 throw new Error('DNS 检测未完成。');
}
export async function requestDns(path:string,signal:AbortSignal):Promise<unknown> {
 const response=await fetch(new URL(path,location.href),{method:path==='dns/start'?'POST':'GET',credentials:'omit',cache:'no-store',redirect:'error',signal:AbortSignal.any([signal,AbortSignal.timeout(25000)])});
 const text=await response.text();if(text.length>32768)throw new Error('DNS 服务响应过大。');
 let value:unknown;try{value=JSON.parse(text);}catch{throw new Error('本地 DNS 接口不可用，请从更新后的 CCBaga-A 重新打开浏览器。');}
 if(!response.ok)throw new Error(typeof (value as {error?:unknown})?.error==='string'?(value as {error:string}).error:'DNS 检测未完成。');
 return value;
}
export function waitDns(ms:number,signal:AbortSignal):Promise<void> {
 signal.throwIfAborted();
 return new Promise((resolve,reject)=>{
  const stop=()=>{clearTimeout(timer);reject(signal.reason);};
  const timer=setTimeout(()=>{signal.removeEventListener('abort',stop);resolve();},ms);
  signal.addEventListener('abort',stop,{once:true});
 });
}
export function triggerDns(id:string,index:number,signal:AbortSignal):Promise<void> {
 signal.throwIfAborted();
 if(!/^[a-f0-9]{48}$/.test(id)||!Number.isInteger(index)||index<1||index>5)throw new Error('DNS 检测轮次无效。');
 return new Promise((resolve,reject)=>{
  const image=new Image();
  const clean=()=>{clearTimeout(timer);signal.removeEventListener('abort',stop);image.onload=image.onerror=null;image.removeAttribute('src');};
  const finish=()=>{clean();resolve();};
  const stop=()=>{clean();reject(signal.reason);};
  const timer=setTimeout(finish,3000);
  signal.addEventListener('abort',stop,{once:true});
  image.referrerPolicy='no-referrer';image.crossOrigin='anonymous';
  image.onload=image.onerror=finish;
  image.src=`https://${id}-${index}.d.ip.net.coffee/pixel.gif`;
 });
}
