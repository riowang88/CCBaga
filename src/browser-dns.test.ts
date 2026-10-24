/**
 * [INPUT]: 已确认的 Net.Coffee token 协议与 B/A 调用替身。
 * [OUTPUT]: B 触发先于 A 读取、轮次隔离、空结果有限等待、失败及取消回归。
 * [POS]: 无外网、无真实用户资料的 DNS 编排测试。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import {test,expect} from 'bun:test';
import {probeDns,type DnsDependencies} from './browser-dns';
const id='a'.repeat(48);
const result={id,resolvers:[{ip:'1.1.1.1',country_code:'AU',country:'Australia',city:'',isp:'Cloudflare',geo_error:null}],observed_at:1};
function fixture(values:unknown[]) {
 const events:string[]=[];let reads=0;
 const deps:DnsDependencies={signal:new AbortController().signal,request:async path=>{events.push(path);if(path==='dns/start')return {id};reads++;const value=values.shift();if(value instanceof Error)throw value;return value;},trigger:async(_id,n)=>{expect(_id).toBe(id);events.push('trigger:'+n);},wait:async()=>{}};
 return {deps,events,reads:()=>reads};
}
test('B 完成本轮五次触发后才让 A 取真实结果',async()=>{
 const f=fixture([result]);expect(await probeDns(f.deps)).toEqual(result);
 expect(f.events).toEqual(['dns/start','trigger:1','trigger:2','trigger:3','trigger:4','trigger:5','dns/result/'+id]);
});
test('空结果最多读取三次，仍为空则保留空观测',async()=>{
 const empty={...result,resolvers:[]};const f=fixture([empty,empty,empty]);expect((await probeDns(f.deps)).resolvers).toEqual([]);expect(f.reads()).toBe(3);
});
test('403 不伪装成功或切换服务，其他轮次结果被拒绝',async()=>{
 const f=fixture([new Error('DNS 服务返回 HTTP 403')]);await expect(probeDns(f.deps)).rejects.toThrow('HTTP 403');expect(f.reads()).toBe(1);
 await expect(probeDns(fixture([{...result,id:'other'}]).deps)).rejects.toThrow('DNS 检测轮次不匹配');
});
test('取消后的轮次不再触发网络请求',async()=>{
 const f=fixture([result]);const controller=new AbortController();controller.abort();f.deps.signal=controller.signal;
 await expect(probeDns(f.deps)).rejects.toThrow();expect(f.events).toEqual([]);
});
