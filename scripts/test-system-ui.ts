/**
 * [INPUT]: 隔离 Playwright 引擎、Vite 与电脑环境页虚构 IPC（tests/system.html）。
 * [OUTPUT]: 电脑环境页基本回归：真实外壳下页面内距与无横向溢出、13 项地区、「同时更换 B 浏览器指纹」标签与说明（aria-describedby）、勾选后命令顺序（B 运行时先换指纹再重开）、英文界面。
 * [POS]: 共用 UI 回归的一部分，不截图、不修改本机、不访问外部服务。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import {strict as assert} from 'node:assert';
import type {Browser} from 'playwright-core';
export async function testSystemUi(browser:Browser,base:string) {
 const context=await browser.newContext({viewport:{width:1200,height:800}});
 await context.route('**/*',route=>route.request().url().startsWith(base+'/')?route.continue():route.abort());
 const page=await context.newPage();page.setDefaultTimeout(10000);
 const errors:string[]=[];page.on('pageerror',error=>errors.push(error.message));
 try{
  await page.goto(base+'/tests/system.html');
  await page.getByRole('heading',{name:'电脑环境',exact:true}).waitFor();
  // ----- 外壳：页面根有左右内距、自己滚动、内容不超出窗口 -----
  const layout=await page.evaluate(()=>{const root=document.querySelector('.system-rb') as HTMLElement;const s=getComputedStyle(root);const right=Math.max(...[...root.querySelectorAll('*')].map(e=>e.getBoundingClientRect().right));return {pad:parseFloat(s.paddingLeft),overflow:s.overflowY,right,width:window.innerWidth};});
  assert.ok(layout.pad>=24,`page padding ${layout.pad}`);assert.equal(layout.overflow,'auto');assert.ok(layout.right<=layout.width,`content right ${layout.right} > ${layout.width}`);
  // ----- 13 项地区，预选当前电脑最接近的新加坡 -----
  const chips=page.getByRole('group',{name:'国家地区'}).getByRole('button');
  assert.equal(await chips.count(),13);
  await page.getByRole('button',{name:'新加坡',exact:true,pressed:true}).waitFor();
  // ----- 勾选项说明对象 -----
  const sync=page.getByRole('checkbox',{name:'同时更换 B 浏览器指纹'});
  assert.equal(await sync.isChecked(),false);
  assert.equal(await page.getByRole('checkbox',{name:/同步/}).count(),0);
  const note=await sync.getAttribute('aria-describedby');
  assert.match(await page.locator('#'+note).innerText(),/新的随机指纹.*语言和时区.*先退出再重开/s);
  // ----- B 运行中：设置地区 → 读 B → 换指纹 → 重开 B -----
  await page.evaluate(()=>{(window as any).systemFixture.browserRunning=true;});
  await sync.check();
  await page.getByRole('button',{name:'日本 · 东京',exact:true}).click();
  await page.getByRole('button',{name:'应用到本机',exact:true}).click();
  await page.waitForFunction(()=>(window as any).systemFixture.commands.some((c:any)=>c.command==='browser_launch'));
  const commands=await page.evaluate(()=>(window as any).systemFixture.commands) as {command:string;args:any}[];
  assert.deepEqual(commands.map(c=>c.command),['system_set_region','browser_read','browser_rotate','browser_launch']);
  assert.deepEqual(commands[2].args,{region:'jp'});
  // ----- 英文界面 -----
  await page.evaluate(()=>(window as any).systemFixture.language('en'));
  await page.getByRole('checkbox',{name:'Also change the B browser fingerprint'}).waitFor();
  assert.deepEqual(errors,[]);
  return 6;
 } finally {await context.close();}
}
