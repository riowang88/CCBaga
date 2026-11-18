/**
 * [INPUT]: 隔离 Playwright 引擎、Vite 与网络页虚构 IPC。
 * [OUTPUT]: 网络环境检测页回归：真实外壳（1040×820）下页面内距/无横向溢出/深色卡无浅色汇总块与白边表格、无 Skill/提示词入口、Claude 可用性四域名待测/检测中/成功/失败与延迟、本机出口、一致性网格、DNS 最近一轮三态与「在 B 中检测」、WebRTC 如实未检测、完整报告链接仅 B 运行时可用、双语 DOM。
 * [POS]: 共用 UI 回归的功能证明，不截图或访问外部服务。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import {strict as assert} from 'node:assert';
import type {Browser} from 'playwright-core';
export async function testNetworkDiagnostics(browser:Browser,base:string) {
 const context=await browser.newContext();
 await context.route('**/*',route=>route.request().url().startsWith(base+'/')?route.continue():route.abort());
 const page=await context.newPage();page.setDefaultTimeout(10000);
 const errors:string[]=[];page.on('pageerror',error=>errors.push(error.message));
 const fixture=(script:string)=>page.evaluate(script);
 const commands=async()=>(await page.evaluate(()=>(window as any).networkFixture.commands)) as {command:string}[];
 try{
  await page.goto(base+'/tests/network.html');
  await page.getByRole('heading',{name:'Claude 可用性',exact:true}).waitFor();
  // ----- 外壳：与真实应用同容器，页面根有内距、自己滚动、主按钮和卡片不超出窗口 -----
  await page.setViewportSize({width:1040,height:820});
  const layout=await page.evaluate(()=>{const root=document.querySelector('.network-page') as HTMLElement;const s=getComputedStyle(root);const right=Math.max(...[...root.querySelectorAll('*')].map(e=>e.getBoundingClientRect().right));return {pad:parseFloat(s.paddingLeft),overflow:s.overflowY,right,width:window.innerWidth,outcomeBg:getComputedStyle(document.querySelector('.domain-outcome')!).backgroundColor,listBorder:getComputedStyle(document.querySelector('.domain-diagnostics-list')!).borderTopWidth};});
  assert.ok(layout.pad>=24,`page padding ${layout.pad}`);assert.equal(layout.overflow,'auto');assert.ok(layout.right<=layout.width,`content right ${layout.right} > ${layout.width}`);
  assert.equal(layout.outcomeBg,'rgba(0, 0, 0, 0)');assert.equal(layout.listBorder,'0px');
  // ----- Skill 入口、提示词与推广均已移除 -----
  assert.equal(await page.getByRole('button',{name:/^(安装 Skill|复制提示词|获取 VPS|获取家宽)$/}).count(),0);
  assert.equal(await page.getByRole('heading',{name:/^(Skill|DMIT|Webshare)$/}).count(),0);
  assert.equal(await page.getByRole('tab').count(),0);
  // ----- B 未运行：WebRTC 不显示通过 -----
  const webrtc=page.locator('.network-webrtc');
  assert.equal(await webrtc.getAttribute('data-webrtc'),'stopped');
  assert.match(await webrtc.innerText(),/不代表通过/);
  assert.equal(await webrtc.locator('.rb-ok').count(),0);
  // ----- B 未运行：完整报告链接禁用，DNS 说明需要 B -----
  const report=page.getByRole('button',{name:'在 B 中打开完整报告 →',exact:true});
  assert.equal(await report.isDisabled(),true);
  const dns=page.locator('.network-dns');
  assert.equal(await dns.getAttribute('data-dns'),'stopped');
  // ----- 一致性网格来自后端 checks，冲突/待测不冒充通过 -----
  const checks=page.locator('.network-check');
  assert.equal(await checks.count(),3);
  assert.equal(await page.locator('.network-check.check-ok').count(),1);
  assert.match(await page.locator('.network-check.check-mismatch').innerText(),/存在冲突/);
  assert.match(await page.locator('.network-check.check-pending').innerText(),/尚未核验/);
  // ----- 四域名探针：待测 → 检测中 → 成功 -----
  const panel=page.locator('.domain-diagnostics'),rows=panel.locator('.domain-diagnostics-item');
  assert.equal(await panel.getAttribute('data-status'),'idle');assert.equal(await rows.count(),4);
  assert.deepEqual(await panel.locator('.domain-name').allInnerTexts(),['claude.ai','api.anthropic.com','platform.claude.com','downloads.claude.ai']);
  await page.getByRole('button',{name:'开始检测',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('.domain-diagnostics')?.getAttribute('data-status')==='checking');
  assert.equal(await panel.locator('.domain-badge.checking').count(),4);
  assert.equal(await page.getByRole('button',{name:'检测中',exact:true}).isDisabled(),true);
  await page.locator('.network-exit[data-status="observed"]').waitFor();
  assert.match(await page.locator('.network-exit').innerText(),/198\.51\.100\.24.*US · Fixture City.*Fixture ISP/s);
  await page.evaluate(()=>(window as any).networkFixture.finish());
  await page.waitForFunction(()=>document.querySelector('.domain-diagnostics')?.getAttribute('data-status')==='passed');
  assert.equal(await panel.locator('.domain-badge.passed').count(),4);
  assert.match(await panel.locator('.domain-outcome').innerText(),/检测成功/);
  assert.match(await rows.first().innerText(),/HTTP 403.*服务可达，需浏览器验证/s);
  assert.match(await rows.first().innerText(),/35 ms/);
  assert.deepEqual((await commands()).map(row=>row.command),['browser_check']);
  for(const mode of ['partial','error','empty','invalid']){
   await page.evaluate(mode=>(window as any).networkFixture.setMode(mode),mode);
   await page.getByRole('button',{name:'全部重新检测',exact:true}).click();
   assert.equal(await panel.locator('.domain-badge.passed').count(),0);
   await page.evaluate(()=>(window as any).networkFixture.finish());
   await page.waitForFunction(()=>document.querySelector('.domain-diagnostics')?.getAttribute('data-status')==='failed');
   assert.match(await panel.locator('.domain-outcome').innerText(),/检测失败/);
   if(mode==='partial'){
    assert.equal(await panel.locator('.domain-badge.failed').count(),1);
    assert.match(await panel.locator('.domain-diagnostics-item.failed').innerText(),/HTTP 403.*受到安全拦截或限制/s);
   }else assert.match(await panel.locator('.domain-outcome').innerText(),mode==='error'?/fixture connection interrupted/:mode==='invalid'?/检测结果无效/:/部分检测未返回结果/);
  }
  // ----- 在 B 中检测：启动 B 后 DNS 呈现最近一轮；空结果与失败如实显示 -----
  await dns.getByRole('button',{name:'打开 B 并检测 →',exact:true}).click();
  await page.locator('.network-dns[data-dns="idle"]').waitFor();
  assert.equal(await report.isEnabled(),true);
  await report.click();
  assert.deepEqual((await commands()).slice(-2),[{command:'browser_launch',args:{webrtc:false}},{command:'browser_open_report',args:null}]);
  await fixture(`window.networkFixture.browser.dns_last={observation:{id:'a'.repeat(48),resolvers:[{ip:'1.1.1.1',country_code:'US',country:'United States',city:'',isp:'Fixture DNS',geo_error:null},{ip:'2001:db8::1',country_code:'',country:'',city:'',isp:'',geo_error:'DNS 服务返回 HTTP 503'}],observed_at:1780000000000},error:'',at:1780000000000};window.networkFixture.refresh();`);
  await page.locator('.network-dns[data-dns="observed"]').waitFor();
  assert.equal(await dns.locator('tbody tr').count(),2);
  assert.match(await dns.innerText(),/共观测到 2 个解析器/);
  // 默认只显示一行结论，明细收在折叠区；展开后才看到逐行归属地
  assert.equal(await dns.locator('details.network-dns-details').getAttribute('open'),null);
  await dns.getByText('查看解析器明细',{exact:true}).click();
  assert.match(await dns.innerText(),/归属地未取得/);
  assert.match(await dns.innerText(),/不自动等于泄露/);
  await fixture(`window.networkFixture.browser.dns_last={observation:{id:'a'.repeat(48),resolvers:[],observed_at:1780000000000},error:'',at:1780000000000};window.networkFixture.refresh();`);
  await page.locator('.network-dns[data-dns="empty"]').waitFor();
  assert.match(await dns.innerText(),/空结果不代表安全/);assert.equal(await dns.locator('tbody tr').count(),0);
  await fixture(`window.networkFixture.browser.dns_last={observation:null,error:'DNS 服务返回 HTTP 403',at:1780000000000};window.networkFixture.refresh();`);
  await page.locator('.network-dns[data-dns="failed"]').waitFor();
  assert.match(await dns.innerText(),/DNS 检测未完成.*HTTP 403/s);
  await dns.getByRole('button',{name:'在 B 中检测 →',exact:true}).waitFor();
  // ----- WebRTC 最近一轮四态：只有一致显示通过色；未知/已禁用为警示；不一致显示 UDP 出口 -----
  assert.equal(await webrtc.getAttribute('data-webrtc'),'idle');
  const webrtcReport=(webrtc:unknown)=>`window.networkFixture.browser.report={language:'en-US',languages:['en-US'],timezone:'America/New_York',platform:'MacIntel',cores:8,user_agent:'fixture',platform_version:'fixture',browser_version:'fixture',geolocation:'denied',exit:{ip:'203.0.113.1',country:'US',city:'',timezone:'America/New_York',isp:''},error:'',webrtc:${JSON.stringify(webrtc)}};window.networkFixture.refresh();`;
  await fixture(webrtcReport({status:'match',udp_ips:['203.0.113.1'],error:''}));
  await page.locator('.network-webrtc[data-webrtc="match"]').waitFor();
  assert.equal(await webrtc.locator('.rb-ok').count(),1);
  await fixture(webrtcReport({status:'mismatch',udp_ips:['198.51.100.7'],error:''}));
  await page.locator('.network-webrtc[data-webrtc="mismatch"]').waitFor();
  assert.match(await webrtc.innerText(),/不一致（可能泄露）.*198\.51\.100\.7/s);
  for(const [status,label] of [['unknown','未知'],['disabled','已禁用']]){
   await fixture(webrtcReport({status,udp_ips:[],error:status==='unknown'?'未取到 STUN 候选（超时、UDP 被拦或浏览器策略限制）。':'WebRTC 已被浏览器策略禁用。'}));
   await page.locator(`.network-webrtc[data-webrtc="${status}"]`).waitFor();
   assert.match(await webrtc.innerText(),new RegExp(label));assert.equal(await webrtc.locator('.rb-ok').count(),0);
  }
  await webrtc.getByRole('button',{name:'在 B 中检测 →',exact:true}).click();
  assert.deepEqual((await commands()).at(-1),{command:'browser_launch',args:{webrtc:true}});
  // ----- 英文界面 -----
  await page.evaluate(()=>(window as any).networkFixture.language('en'));
  await page.getByRole('heading',{name:'Claude availability',exact:true}).waitFor();
  assert.match(await panel.locator('.domain-outcome').innerText(),/Check failed/);
  await page.getByRole('button',{name:'Run all checks again',exact:true}).waitFor();
  await page.getByRole('button',{name:'Open full report in B →',exact:true}).waitFor();
  assert.match(await webrtc.innerText(),/Unknown|Disabled|Consistent|Inconsistent|Check/);
  assert.deepEqual(errors,[]);
  return 11;
 } finally {await context.close();}
}
