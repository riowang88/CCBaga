/**
 * [INPUT]: 隔离 Chromium、Vite 与虚构 A/外部 DNS 响应。
 * [OUTPUT]: 页面显示 IPv4/IPv6、部分未知、空结果、403、取消旧轮次与断连的回归。
 * [POS]: 不访问外网或用户资料；只测试 browser.html 真实组件。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import {strict as assert} from 'node:assert';
import type {Browser} from 'playwright-core';
export async function testBrowserDns(browser:Browser,base:string) {
 const context=await browser.newContext({locale:'en-US',timezoneId:'America/Los_Angeles'});
 const page=await context.newPage();page.setDefaultTimeout(15000);
 let mode:'success'|'empty'|'failure'|'stale'='success',round=0,reads=0,images=0,language='zh',disconnected=false;
 let release!:()=>void,ready!:()=>void;
 const gate=new Promise<void>(r=>release=r),waiting=new Promise<void>(r=>ready=r);
 const result=(id:string,ip='1.1.1.1')=>({id,observed_at:Date.now(),resolvers:[{ip,country_code:'AU',country:'Australia',city:'',isp:'Cloudflare',geo_error:null},{ip:'2001:4860:4860::8888',country_code:'',country:'',city:'',isp:'',geo_error:'DNS 服务返回 HTTP 503'}]});
 await context.route('**/*',async route=>{
  const request=route.request(),url=request.url();
  if(/^https:\/\/[a-f0-9]{48}-[1-5]\.d\.ip\.net\.coffee\/pixel\.gif$/.test(url)){
   assert.equal(request.headers().referer,undefined);assert.equal(request.headers().cookie,undefined);images++;return route.fulfill({status:204,body:''});
  }
  if(!url.startsWith(base+'/'))throw new Error('Unexpected external request '+url);
  if(url===base+'/context'){
   if(disconnected)return route.fulfill({status:503,body:'unavailable'});
   return route.fulfill({json:{identity:{linked:true,status:'empty',name:'Fixture'},ui_language:language,exit_services:[base+'/fixture-exit'],region:{country:'US',label:'United States'},config:{follow_machine:false,language:'en-US',timezone:'America/Los_Angeles'},machine:{platform:'macos',version:'',cores:1,timezone:'America/Los_Angeles',languages:['en-US'],locale:'en_US',locale_country:'US',location_enabled:false},engine_version:'1'}});
  }
  if(url===base+'/fixture-exit')return route.fulfill({json:{ip:'203.0.113.1',country_code:'US',timezone:'America/Los_Angeles'}});
  if(url===base+'/report')return route.fulfill({json:{ok:true}});
  if(url===base+'/dns/start'){assert.equal(request.method(),'POST');return route.fulfill({json:{id:(++round).toString(16).padStart(48,'0')}});}
  if(url.includes('/dns/result/')){
   reads++;const id=url.split('/').at(-1)!;
   if(mode==='failure')return route.fulfill({status:502,json:{error:'DNS 服务返回 HTTP 403'}});
   if(mode==='empty')return route.fulfill({json:{...result(id),resolvers:[]}});
   if(mode==='stale'){ready();await gate;try{return await route.fulfill({json:result(id,'9.9.9.9')});}catch{return;}}
   return route.fulfill({json:result(id)});
  }
  return route.continue();
 });
 try{
  await page.goto(base+'/browser.html');
  const panel=page.locator('.verification-dns');
  await page.locator('[data-dns="observed"]').waitFor();
  assert.equal(images,5);assert.equal(reads,1);
  assert.match(await panel.innerText(),/1\.1\.1\.1/);assert.match(await panel.innerText(),/2001:4860:4860::8888/);assert.match(await panel.innerText(),/归属地未取得/);
  assert.equal(await page.locator('.verification-check').count(),12);
  mode='empty';const before=reads;
  await panel.getByRole('button',{name:'重新检测 DNS',exact:true}).click();
  await page.locator('[data-dns="checking"]').waitFor();assert.equal(await panel.locator('tbody tr').count(),0);
  await page.locator('[data-dns="empty"]').waitFor();assert.equal(reads-before,3);assert.match(await panel.innerText(),/空结果不代表安全/);assert.equal(await panel.locator('tbody tr').count(),0);
  mode='failure';language='en';
  await page.getByRole('button',{name:'重新实测',exact:true}).click();
  await page.locator('[data-dns="failed"]').waitFor();assert.match(await panel.innerText(),/The DNS service returned HTTP 403/);
  mode='stale';await panel.getByRole('button',{name:'Check DNS again',exact:true}).click();await waiting;
  mode='success';await page.getByRole('button',{name:'Run check',exact:true}).click();
  await page.locator('[data-dns="observed"]').waitFor();release();
  assert.equal(await panel.getByText('9.9.9.9',{exact:true}).count(),0);assert.match(await panel.innerText(),/1\.1\.1\.1/);
  disconnected=true;await page.getByRole('button',{name:'Run check',exact:true}).click();
  await page.locator('[data-dns="disconnected"]').waitFor();assert.equal(await panel.locator('tbody tr').count(),0);assert.equal(await panel.getByRole('button',{name:'Check DNS again',exact:true}).isDisabled(),true);
  return 6;
 }finally{release();await context.close();}
}
