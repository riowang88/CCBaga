/**
 * [INPUT]: 真实身份界面与虚构 IPC。
 * [OUTPUT]: 共享默认关闭、保留历史、持久化、显式同步/层级补齐计数/失败及双语回归。
 * [POS]: 隔离浏览器测试，不读写真实会话目录。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import {strict as assert} from 'node:assert';
import type {Browser} from 'playwright-core';
export async function testDesktopSessions(browser:Browser,base:string){
 const context=await browser.newContext();
 await context.route('**/*',route=>route.request().url().startsWith(base+'/')?route.continue():route.abort());
 const page=await context.newPage();page.setDefaultTimeout(10000);
 try{
  await page.goto(base+'/tests/identities.html');
  await page.locator('.identity-policy summary').click();
  const share=page.getByRole('checkbox',{name:'跨账号共享桌面 Code 会话',exact:true});
  const history=page.getByRole('checkbox',{name:'会话历史',exact:true});
  const sync=page.getByRole('button',{name:'同步桌面会话',exact:true});
  assert.equal(await share.isChecked(),false);assert.equal(await sync.isDisabled(),true);
  await share.click();assert.equal(await history.isChecked(),true);assert.equal(await history.isDisabled(),true);
  assert.equal(await page.evaluate(()=>(window as any).identityFixture.commands.length),0);
  await sync.click();await page.getByRole('status').filter({hasText:'新增 3 个入口，更新 2 个会话'}).waitFor();
  assert.equal(await page.evaluate(()=>(window as any).identityFixture.commands.filter((r:any)=>r.command==='identities_sync_desktop_sessions').length),1);
  await page.reload();await page.locator('.identity-policy summary').click();assert.equal(await share.isChecked(),true);
  await page.evaluate(()=>(window as any).identityFixture.setFailure('identities_sync_desktop_sessions'));
  await sync.click();await page.getByRole('alert').filter({hasText:'fixture operation failed'}).waitFor();
  assert.equal(await page.getByRole('status').filter({hasText:'新增 3 个入口'}).count(),0);
  await page.evaluate(()=>(window as any).identityFixture.sessionConflict=true);
  await sync.click();let dialog=page.getByRole('alertdialog');await dialog.waitFor();
  const proceed=dialog.getByRole('button',{name:'使用所选版本继续',exact:true});assert.equal(await proceed.isDisabled(),true);
  assert.equal(await dialog.getByRole('radio').count(),2);
  assert.equal(await dialog.getByRole('button',{name:'取消',exact:true}).evaluate(el=>el===document.activeElement),true);
  await dialog.getByRole('button',{name:'取消',exact:true}).click();
  assert.equal(await page.evaluate(()=>(window as any).identityFixture.sessionConflict),true);
  await sync.click();await dialog.getByRole('radio').nth(1).check();await proceed.click();
  await page.getByRole('status').filter({hasText:'新增 3 个入口，更新 2 个会话'}).waitFor();
  const selected=await page.evaluate(()=>(window as any).identityFixture.commands.filter((r:any)=>r.command==='identities_sync_desktop_sessions').at(-1).args.sessionChoices);
  assert.deepEqual(selected,[{session:'local_fixture.json',revision:'fixture-revision',cli_id:'branch-b'}]);
  await page.evaluate(()=>(window as any).identityFixture.sessionConflict=true);
  await page.getByRole('button',{name:'使用空身份',exact:true}).click();
  await page.getByRole('dialog').getByRole('button',{name:'保存当前身份并切换',exact:true}).click();
  await dialog.waitFor();assert.equal(await proceed.isDisabled(),true);
  await dialog.getByRole('radio').nth(1).check();await proceed.click();
  await page.waitForFunction(()=>(window as any).identityFixture.sessionConflict===false);
  assert.equal(await page.evaluate(()=>(window as any).identityFixture.commands.filter((r:any)=>r.command==='identities_switch').at(-1).args.sessionChoices[0].cli_id),'branch-b');
  await page.getByRole('status').filter({hasText:'身份切换完成。'}).waitFor();
  await share.click();assert.equal(await history.isChecked(),true);assert.equal(await history.isDisabled(),false);assert.equal(await sync.isDisabled(),true);
  await page.reload();await page.locator('.identity-policy summary').click();assert.equal(await share.isChecked(),false);
  await page.evaluate(()=>(window as any).identityFixture.language('en'));
  await page.getByRole('checkbox',{name:'Share desktop Code sessions across accounts',exact:true}).waitFor();
  return 12;
 }finally{await context.close();}
}
