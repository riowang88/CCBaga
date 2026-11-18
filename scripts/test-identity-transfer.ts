/**
 * [INPUT]: 隔离浏览器与虚构身份 IPC、原生选择票据和真实 Channel 回调。
 * [OUTPUT]: 导入/导出确认、重选/取消清除来源确认、密码/失败不覆盖、实际进度（含重复身份预检阶段与对话框内冲突错误）及到期与进程未知状态回归。
 * [POS]: 身份迁移 GUI 的行为验收，无真实凭据/文件/原生命令或截图。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import {strict as assert} from 'node:assert';
import type {Browser} from 'playwright-core';
export async function testIdentityTransfer(browser:Browser,base:string){
 const context=await browser.newContext({viewport:{width:1040,height:900},reducedMotion:'reduce'});
 await context.route('**/*',route=>route.request().url().startsWith(base+'/')?route.continue():route.abort());
 const page=await context.newPage();page.setDefaultTimeout(10000);let checks=0;
 try{
  await page.clock.install();await page.goto(base+'/tests/identities.html');
  const current=page.locator('.identity-card.current');await current.getByText('本地未过期',{exact:true}).waitFor();
  await page.evaluate(()=>{const fixture=(window as any).identityFixture;fixture.state.current.expires_at=Date.now()+1000;});
  await page.getByRole('button',{name:'刷新',exact:true}).click();await page.clock.fastForward(1100);
  await current.getByText('本地已到期',{exact:true}).waitFor();assert.equal(await current.locator('.auth-local_unexpired').count(),0);checks++;
  await page.evaluate(()=>{const fixture=(window as any).identityFixture;fixture.state.processes=null;fixture.state.processes_truncated=false;fixture.state.current.expires_at=null;});
  await page.getByRole('button',{name:'刷新',exact:true}).click();await current.locator('.auth-unknown').waitFor();
  await current.getByRole('button',{name:'更新备份',exact:true}).click();
  let dialog=page.getByRole('dialog');await dialog.getByText('进程状态未知；执行前将重新核验。',{exact:true}).waitFor();
  assert.equal(await dialog.getByText('未检测到 Claude 进程。',{exact:true}).count(),0);await dialog.getByRole('button',{name:'取消',exact:true}).click();checks++;
  await current.getByRole('button',{name:/^更多操作/}).click();await page.getByRole('menuitem',{name:'导出保存备份',exact:true}).click();dialog=page.getByRole('dialog');await dialog.getByText('备份时间',{exact:true}).waitFor();assert.equal(await dialog.getByText('此备份未关联浏览器配置。',{exact:true}).count(),1);await dialog.getByRole('button',{name:'取消',exact:true}).click();checks++;
  await page.evaluate(()=>{(window as any).identityFixture.state.browser.bound_ids.push('00000000000000000000000000000002');});await page.getByRole('button',{name:'刷新',exact:true}).click();
  const saved=page.locator('.identity-card').filter({has:page.getByRole('heading',{name:'Identity 2',exact:true})});
  await saved.getByRole('button',{name:/^更多操作/}).click();await page.getByRole('menuitem',{name:'导出身份 Identity 2',exact:true}).click();dialog=page.getByRole('dialog');
  const submit=dialog.getByRole('button',{name:'加密导出',exact:true});assert.equal(await submit.isEnabled(),false);
  await page.evaluate(()=>{(window as any).identityFixture.transfer.cancel=true;});
  await dialog.getByRole('button',{name:'选择保存位置',exact:true}).click();assert.equal(await dialog.getByText('尚未选择文件',{exact:true}).count(),1);
  await dialog.getByRole('button',{name:'选择保存位置',exact:true}).click();await dialog.getByText('fixture.bagapack',{exact:true}).waitFor();
  assert.equal(await dialog.getByRole('checkbox',{name:'包含浏览器指纹配置（不含网页登录资料）',exact:true}).isChecked(),false);
  await dialog.getByLabel('迁移密码（至少 12 个字符）',{exact:true}).fill('short');await dialog.getByLabel('确认迁移密码',{exact:true}).fill('short');
  await dialog.getByRole('checkbox',{name:'我会妥善保管迁移文件和密码',exact:true}).check();assert.equal(await submit.isEnabled(),false);checks++;
  // 禁用原因就地可见：不足 12 字符标红并提示差几个，页脚列出仍缺的项
  const password=dialog.getByLabel('迁移密码（至少 12 个字符）',{exact:true});
  assert.equal(await password.getAttribute('aria-invalid'),'true');
  assert.match(await dialog.locator('#identity-transfer-password-hint').innerText(),/还差 7 个字符/);
  assert.match(await dialog.locator('.identity-transfer-missing').innerText(),/还需：密码不足 12 个字符/);
  await password.fill('synthetic-transfer-password');
  assert.match(await dialog.locator('.identity-transfer-missing').innerText(),/两次密码不一致/);
  assert.equal(await dialog.getByLabel('确认迁移密码',{exact:true}).getAttribute('aria-invalid'),'true');checks++;
  await dialog.getByLabel('迁移密码（至少 12 个字符）',{exact:true}).fill('synthetic-transfer-password');await dialog.getByLabel('确认迁移密码',{exact:true}).fill('synthetic-transfer-password');
  await page.evaluate(()=>{(window as any).identityFixture.transfer.cancel=true;});await dialog.getByRole('button',{name:'选择保存位置',exact:true}).click();
  await dialog.getByText('尚未选择文件',{exact:true}).waitFor();assert.equal(await submit.isEnabled(),false);assert.equal(await dialog.getByRole('checkbox',{name:'我会妥善保管迁移文件和密码',exact:true}).isChecked(),false);checks++;
  await dialog.getByRole('button',{name:'选择保存位置',exact:true}).click();await dialog.getByText('fixture.bagapack',{exact:true}).waitFor();
  await page.evaluate(()=>{(window as any).identityFixture.transfer.failPick=true;});await dialog.getByRole('button',{name:'选择保存位置',exact:true}).click();
  await dialog.getByRole('alert').waitFor();assert.equal(await submit.isEnabled(),false);assert.equal(await dialog.getByText('尚未选择文件',{exact:true}).count(),1);checks++;
  await dialog.getByRole('button',{name:'选择保存位置',exact:true}).click();await dialog.getByText('fixture.bagapack',{exact:true}).waitFor();
  await dialog.getByRole('checkbox',{name:'我会妥善保管迁移文件和密码',exact:true}).check();
  await dialog.getByRole('checkbox',{name:'包含浏览器指纹配置（不含网页登录资料）',exact:true}).check();
  const active=await page.evaluate(()=>(window as any).identityFixture.state.active);
  await page.evaluate(()=>{(window as any).identityFixture.defer('identities_export');});await submit.click();
  await page.waitForFunction(()=>(window as any).identityFixture.pending.includes('identities_export'));
  assert.equal(await dialog.getByLabel('迁移密码（至少 12 个字符）',{exact:true}).inputValue(),'');
  assert.equal(await dialog.getByRole('button',{name:'取消',exact:true}).isEnabled(),false);
  await page.evaluate(()=>{(window as any).identityFixture.emitProgress('identities_export',{stage:'transfer_archive',status:'running',message:'已写入迁移文件。',current:1,total:4,elapsed_ms:0});});
  await page.locator('.identity-operation [data-stage="transfer_archive"]').waitFor();assert.equal(await page.locator('.identity-operation [role="progressbar"]').getAttribute('aria-valuenow'),'25');
  await page.evaluate(()=>{(window as any).identityFixture.release('identities_export');});await dialog.waitFor({state:'detached'});
  const request=await page.evaluate(()=>((window as any).identityFixture.commands as any[]).find(row=>row.command==='identities_export').args);
  assert.equal(request.includeBrowser,true);assert.equal(request.grant,'fixture-grant');assert.equal(request.id,'00000000000000000000000000000002');
  assert.equal(await page.evaluate(()=>(window as any).identityFixture.state.active),active);checks++;
  await page.getByRole('button',{name:'导入',exact:true}).click();dialog=page.getByRole('dialog');
  assert.equal(await dialog.getByLabel('迁移密码（至少 12 个字符）',{exact:true}).inputValue(),'');
  await dialog.getByRole('button',{name:'选择文件',exact:true}).click();await dialog.getByLabel('迁移密码（至少 12 个字符）',{exact:true}).fill('synthetic-transfer-password');
  await dialog.getByRole('checkbox',{name:'我确认备份来源可信，并了解恢复配置或插件可能执行其命令。',exact:true}).check();
  await dialog.getByRole('button',{name:'选择文件',exact:true}).click();await dialog.getByText('fixture.bagapack',{exact:true}).waitFor();
  assert.equal(await dialog.getByRole('checkbox',{name:'我确认备份来源可信，并了解恢复配置或插件可能执行其命令。',exact:true}).isChecked(),false);
  assert.equal(await dialog.getByRole('button',{name:'解密导入',exact:true}).isEnabled(),false);
  await dialog.getByRole('checkbox',{name:'我确认备份来源可信，并了解恢复配置或插件可能执行其命令。',exact:true}).check();
  await page.evaluate(()=>{(window as any).identityFixture.setFailure('identities_import');});await dialog.getByRole('button',{name:'解密导入',exact:true}).click();
  await dialog.getByRole('alert').waitFor();assert.equal(await dialog.getByText('尚未选择文件',{exact:true}).count(),1);
  assert.equal(await dialog.getByLabel('迁移密码（至少 12 个字符）',{exact:true}).inputValue(),'');assert.equal(await page.evaluate(()=>(window as any).identityFixture.state.active),active);checks++;
  await dialog.getByRole('button',{name:'选择文件',exact:true}).click();await dialog.getByLabel('迁移密码（至少 12 个字符）',{exact:true}).fill('synthetic-transfer-password');
  await dialog.getByRole('checkbox',{name:'我确认备份来源可信，并了解恢复配置或插件可能执行其命令。',exact:true}).check();
  await dialog.getByRole('button',{name:'解密导入',exact:true}).click();await dialog.waitFor({state:'detached'});
  await page.getByRole('heading',{name:'Imported fixture',exact:true}).waitFor();assert.equal(await page.evaluate(()=>(window as any).identityFixture.state.active),active);checks++;
  await page.getByRole('button',{name:'导入',exact:true}).click();dialog=page.getByRole('dialog');
  await dialog.getByRole('button',{name:'选择文件',exact:true}).click();await dialog.getByLabel('迁移密码（至少 12 个字符）',{exact:true}).fill('synthetic-transfer-password');await dialog.getByRole('checkbox',{name:'我确认备份来源可信，并了解恢复配置或插件可能执行其命令。',exact:true}).check();
  await page.evaluate(()=>{(window as any).identityFixture.transfer.failAfterImport=true;});await dialog.getByRole('button',{name:'解密导入',exact:true}).click();
  await dialog.getByRole('alert').waitFor();await page.waitForFunction(()=>[...document.querySelectorAll('.identity-card h3')].filter(node=>node.textContent==='Imported fixture').length===2);assert.equal(await page.locator('.identity-card h3').filter({hasText:'Imported fixture'}).count(),2);assert.equal(await page.evaluate(()=>(window as any).identityFixture.state.active),active);
  assert.equal(await page.locator('.identity-operation').getAttribute('data-status'),'error');await dialog.getByRole('button',{name:'取消',exact:true}).click();checks++;
  // 重复账号在预检阶段失败：真实阶段名显示为「检查重复身份」，不出现写入阶段，错误留在对话框内并指明冲突档案。
  await page.getByRole('button',{name:'导入',exact:true}).click();dialog=page.getByRole('dialog');
  await dialog.getByRole('button',{name:'选择文件',exact:true}).click();await dialog.getByLabel('迁移密码（至少 12 个字符）',{exact:true}).fill('synthetic-transfer-password');await dialog.getByRole('checkbox',{name:'我确认备份来源可信，并了解恢复配置或插件可能执行其命令。',exact:true}).check();
  await page.evaluate(()=>{(window as any).identityFixture.defer('identities_import');});await dialog.getByRole('button',{name:'解密导入',exact:true}).click();
  await page.waitForFunction(()=>(window as any).identityFixture.pending.includes('identities_import'));
  await page.evaluate(()=>{const f=(window as any).identityFixture;f.emitProgress('identities_import',{stage:'transfer_validate',status:'ok',message:'迁移包已通过完整认证与范围检查。',current:3,total:3,elapsed_ms:0});f.emitProgress('identities_import',{stage:'transfer_precheck',status:'running',message:'正在检查本机是否已有同账号或同浏览器指纹。',current:0,total:null,elapsed_ms:0});});
  const precheck=page.locator('.identity-operation [data-stage="transfer_precheck"]');await precheck.waitFor();assert.match(await precheck.innerText(),/检查重复身份/);
  const duplicate='已保存身份「Imported fixture」是同一账号；未覆盖，导入已停止。可删除该身份后重新导入，或直接使用它。';
  await page.evaluate(message=>{(window as any).identityFixture.release('identities_import',message);},duplicate);
  await dialog.getByRole('alert').filter({hasText:'Imported fixture'}).waitFor();assert.match(await dialog.getByRole('alert').innerText(),/已保存身份「Imported fixture」是同一账号/);
  assert.equal(await page.locator('.identity-operation [data-stage="transfer_archive"]').count(),0);assert.equal(await page.locator('.identity-operation').getAttribute('data-status'),'error');
  assert.equal(await page.evaluate(()=>(window as any).identityFixture.state.active),active);await dialog.getByRole('button',{name:'取消',exact:true}).click();checks++;
  await page.evaluate(()=>{(window as any).identityFixture.language('en');});await page.getByRole('button',{name:'Import',exact:true}).click();dialog=page.getByRole('dialog');
  await dialog.getByLabel('Transfer password (at least 12 characters)',{exact:true}).waitFor();assert.match(await dialog.innerText(),/Same operating system/);checks++;
  return checks;
 }finally{await context.close();}
}
