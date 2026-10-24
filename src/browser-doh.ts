/**
 * [INPUT]: 当前身份 DNS 配置与前后端共用的服务商目录。
 * [OUTPUT]: 沿用/严格 DoH 两种模式、地址预览和输入错误；不发起解析或写系统设置。
 * [POS]: A 的配置表单模型；原生层独立校验，实际 DNS 仍以观测为准。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import catalog from './browser-doh-providers.json?raw';
export type DnsConfig={mode:'inherit'|'secure';provider:'cloudflare'|'google'|'quad9'|'custom';custom_url:string};
export const DEFAULT_DNS:DnsConfig={mode:'inherit',provider:'cloudflare',custom_url:''};
export const DOH_PROVIDERS=JSON.parse(catalog) as ReadonlyArray<{id:Exclude<DnsConfig['provider'],'custom'>;name:string;url:string}>;
export function dohEndpoint(config:DnsConfig):string {return config.provider==='custom'?config.custom_url:DOH_PROVIDERS.find(p=>p.id===config.provider)?.url||'';}
export function validateDoh(config:DnsConfig):string|null {
 if(config.mode==='inherit')return null;
 const invalid='填写单个有效的 HTTPS DoH 地址；不支持用户名、密码、片段或空白，可使用末尾 {?dns} / {&dns} 模板。';
 const value=dohEndpoint(config);
 if(!value||new TextEncoder().encode(value).length>2048||/[\s\x00-\x1f\x7f\\]/.test(value))return invalid;
 const suffix=value.endsWith('{?dns}')?'{?dns}':value.endsWith('{&dns}')?'{&dns}':'';
 const base=suffix?value.slice(0,-suffix.length):value;
 if(/[{}]/.test(base))return invalid;
 try {const url=new URL(base);if(url.protocol!=='https:'||!url.hostname||url.username||url.password||url.hash||base.includes('#')||(suffix==='{?dns}'&&base.includes('?'))||(suffix==='{&dns}'&&!base.includes('?')))return invalid;}catch{return invalid;}
 return null;
}
