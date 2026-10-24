/**
 * [INPUT]: 隔离 Chromium、现有内存 IPC 入口和前后端共用 DoH 服务商目录。
 * [OUTPUT]: zh-Hans-SG、模式/服务商/自定义地址、保存回读、范围提示与运行中锁定。
 * [POS]: 只操作虚构配置，不调用原生或修改用户网络设置。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import {strict as assert} from 'node:assert';
import type {Browser} from 'playwright-core';
export async function testBrowserDoh(browser:Browser,base:string) {
 const context=await browser.newContext();await context.route('**/*',route=>route.request().url().startsWith(base+'/')?route.continue():route.abort());
 const page=await context.newPage();page.setDefaultTimeout(10000);
 try {
  await page.goto(base+'/tests/identities.html?panel=browser');
  const save=page.getByRole('button',{name:'保存设置',exact:true});
  const choose=async(name:string,option:string)=>{await page.getByRole('combobox',{name,exact:true}).click();await page.getByRole('option',{name:option,exact:true}).click();};
  assert.match(await page.locator('.browser-dns-settings').innerText(),/沿用现有解析.*原有自动模式可能回退普通 DNS/s);
  assert.equal(await page.getByRole('combobox',{name:'DoH 服务商',exact:true}).count(),0);
  await page.getByRole('checkbox',{name:'语言与时区跟随机器',exact:true}).uncheck();
  await choose('浏览器语言','zh-Hans-SG');
  await choose('DNS 解析方式','指定 DoH');
  assert.match(await page.locator('.browser-doh-endpoint').innerText(),/https:\/\/cloudflare-dns.com\/dns-query/);
  await choose('DoH 服务商','Google Public DNS');assert.match(await page.locator('.browser-doh-endpoint').innerText(),/dns.google/);
  await choose('DoH 服务商','Quad9（恶意域名过滤）');assert.match(await page.locator('.browser-doh-endpoint').innerText(),/dns.quad9.net/);
  await choose('DoH 服务商','自定义地址');assert(await save.isDisabled());
  const address=page.getByRole('textbox',{name:'DoH HTTPS 地址',exact:true});
  await address.fill('http://resolver.invalid/dns-query');assert(await save.isDisabled());
  await address.fill('https://resolver.example/dns-query{?dns}');assert(await save.isEnabled());
  await save.click();
  const config=await page.evaluate(()=>(window as any).identityFixture.browser.config);
  assert.equal(config.language,'zh-Hans-SG');assert.deepEqual(config.dns,{mode:'secure',provider:'custom',custom_url:'https://resolver.example/dns-query{?dns}'});
  assert(await save.isDisabled());
  await choose('DNS 解析方式','沿用现有解析');await save.click();
  assert.equal(await page.evaluate(()=>(window as any).identityFixture.browser.config.dns.mode),'inherit');
  // 非 DNS 编辑仍保留停用时的自定义地址，切回严格模式可继续编辑。
  await choose('DNS 解析方式','指定 DoH');assert.equal(await address.inputValue(),'https://resolver.example/dns-query{?dns}');
  await page.evaluate(()=>(window as any).identityFixture.browser.running=true);await page.getByRole('button',{name:'刷新状态',exact:true}).click();
  assert(await page.getByRole('combobox',{name:'DNS 解析方式',exact:true}).isDisabled());assert(await address.isDisabled());assert(await save.isDisabled());
  await page.evaluate(()=>(window as any).identityFixture.language('en'));
  assert.match(await page.locator('.browser-dns-settings').innerText(),/Strict DoH.*do not automatically fall back/s);
  await page.getByText('Scope and fallback',{exact:true}).click();
  assert.match(await page.locator('.browser-dns-settings').innerText(),/HTTP\/SOCKS proxy.*Bootstrapping/s);
  return 4;
 }finally{await context.close();}
}
