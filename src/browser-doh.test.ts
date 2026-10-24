import {test,expect} from 'bun:test';
import {DEFAULT_DNS,DOH_PROVIDERS,validateDoh,dohEndpoint} from './browser-doh';
test('沿用旧配置不强制启用 DNS，严格模式选择单个官方端点',()=>{
 expect(validateDoh(DEFAULT_DNS)).toBeNull();expect(DEFAULT_DNS.mode).toBe('inherit');
 expect(DOH_PROVIDERS.map(p=>p.id)).toEqual(['cloudflare','google','quad9']);
 for(const p of DOH_PROVIDERS){const config={...DEFAULT_DNS,mode:'secure' as const,provider:p.id};expect(dohEndpoint(config)).toBe(p.url);expect(validateDoh(config)).toBeNull();}
});
test('自定义 HTTPS POST/GET 和私有解析器有效，坏输入不能保存为严格模式',()=>{
 for(const url of ['https://resolver.example/dns-query','https://[::1]:8443/dns-query','https://resolver.example/id?key=value{&dns}','https://resolver.example/dns-query{?dns}'])expect(validateDoh({...DEFAULT_DNS,mode:'secure',provider:'custom',custom_url:url})).toBeNull();
 for(const url of ['','http://resolver.example/dns-query','https://user:pass@resolver.example/','https://resolver.example/#','https://resolver.example/{other}','https://resolver.example/\n--flag','https://a.example/ https://b.example/'])expect(validateDoh({...DEFAULT_DNS,mode:'secure',provider:'custom',custom_url:url})).not.toBeNull();
 expect(validateDoh({...DEFAULT_DNS,provider:'custom',custom_url:''})).toBeNull();
});
