// Offline UI regressions for retained policy, local selections and explicit client/browser status labels.
import {strict as assert} from 'node:assert';
import {resolve} from 'node:path';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createServer} from 'vite';
import {chromium} from 'playwright-core';

const server=await createServer({server:{host:'127.0.0.1',port:0,watch:{ignored:['**/src-tauri/**','**/browser-runtime/**']}},optimizeDeps:{include:['@tauri-apps/api/mocks']}});
await server.listen();
const port=(server.httpServer!.address() as {port:number}).port;
const base=`http://127.0.0.1:${port}`;
const temporary=await mkdtemp(join(tmpdir(),'ccbaga-regression-'));
const engine=process.env.CCBAGA_TEST_ENGINE??resolve('browser-runtime/Browser.app/Contents/MacOS/Chromium');
const browser=await chromium.launch({executablePath:engine,headless:true,env:{...process.env,HOME:temporary}});
let failures=0;
try {
 for(const name of ['policy','selection','status-labels','browser-only']) {
  const context=await browser.newContext();
  await context.route('**/*',route=>route.request().url().startsWith(base+'/')?route.continue():route.abort());
  const page=await context.newPage();
  page.setDefaultTimeout(3000);
  try {
   await page.goto(base+'/tests/identities.html');
   await page.getByText('fixture@example.invalid',{exact:true}).waitFor();
   if(name==='policy') {
    await page.getByText('保留在工作区',{exact:true}).click();
    await page.getByRole('checkbox',{name:'会话历史',exact:true}).check();
    await page.reload();
    await page.getByText('fixture@example.invalid',{exact:true}).waitFor();
    await page.getByText('保留在工作区',{exact:true}).click();
    assert.equal(await page.getByRole('checkbox',{name:'会话历史',exact:true}).isChecked(),true,'history retention must survive reopening');
    for(const label of ['Skills 与命令','插件'])assert.equal(await page.getByRole('checkbox',{name:label,exact:true}).isChecked(),true);
    await page.locator('.identity-card.current').getByRole('button',{name:'更新备份',exact:true}).click();
    assert.match(await page.getByRole('dialog').locator('.identity-confirm-keep').filter({hasText:'留在工作区'}).innerText(),/会话历史/,'opening backup must not overwrite the explicit retention choice');
   } else if(name==='selection') {
    await page.evaluate(()=>{
     const s=(window as any).identityFixture.state;
     s.active='current';s.last_selected=s.records[1].id;s.workspace_id=s.records[1].id;
     s.current={present:true,display_name:null,email:null,organization:null,unavailable:false,auth_status:'unknown'};
    });
    await page.getByRole('button',{name:'刷新',exact:true}).click();
    const current=page.locator('.identity-card.current');
    assert.equal(await current.getByRole('heading').innerText(),'Identity 2','committed local selection must remain visible without Code metadata');
    assert.equal(await page.getByRole('heading',{name:'Identity 2',exact:true}).count(),1,'current archive must appear exactly once');
    assert.equal(await page.locator('.identity-card').count(),41,'40 archive cards plus empty, without a synthetic current duplicate');
    assert.match(await current.innerText(),/正在使用/);
    assert.equal(await current.getByRole('button',{name:'重命名',exact:true}).count(),1,'the active archive must retain explicit renaming');
    assert.match(await current.innerText(),/CLI 账号未识别/);
    assert.doesNotMatch(await current.innerText(),/本地未过期|未备份/,'selection must not claim credential validity or absence of a backup');
    await page.locator('.identity-card').filter({has:page.getByRole('heading',{name:'Identity 3',exact:true})}).getByRole('button',{name:'切换',exact:true}).click();
    const dialog=page.getByRole('dialog');
    assert.equal(await dialog.locator('#identity-current-name').count(),0,'updating an existing archive must not ask to save as another identity');
    assert.match(await dialog.innerText(),/保存当前身份.*Identity 2/s);
    assert.match(await dialog.innerText(),/切换到.*Identity 3/s);
    await dialog.getByRole('button',{name:'保存当前身份并切换',exact:true}).click();
    const command=await page.evaluate(()=>(window as any).identityFixture.commands.findLast((row:any)=>row.command==='identities_switch'));
    assert.equal(command.args.currentName,'Identity 2');
    await page.evaluate(()=>{
     const s=(window as any).identityFixture.state;
     s.workspace_id=null;s.active='current';
     s.current={present:true,display_name:'External account',email:'external@example.invalid',organization:null,unavailable:false,auth_status:'unknown'};
    });
    await page.getByRole('button',{name:'刷新',exact:true}).click();
    assert.equal(await current.getByRole('heading').innerText(),'External account');
    assert.doesNotMatch(await current.innerText(),/正在使用/,'an observed external account must not be labeled as the last selection');
    await page.locator('.identity-card').filter({has:page.getByRole('heading',{name:'Identity 3',exact:true})}).getByRole('button',{name:'切换',exact:true}).click();
    assert.equal(await page.getByRole('dialog').locator('#identity-current-name').count(),1,'an unbound identity still needs a first archive name');
   } else if(name==='browser-only') {
    await page.evaluate(()=>{const s=(window as any).identityFixture.state;s.active='empty';s.workspace_id=null;s.browser_profile_present=true;s.current={present:false,display_name:null,email:null,organization:null,unavailable:false,auth_status:'unknown'};});
    await page.getByRole('button',{name:'刷新',exact:true}).click();
    const empty=page.locator('.identity-card.empty');
    await page.locator('.identity-card.current').getByRole('button',{name:'保存身份',exact:true}).click();
    const dialog=page.getByRole('dialog');
    assert.match(await dialog.innerText(),/浏览器资料/);
    await dialog.locator('#identity-current-name').fill('rio.wt');
    await dialog.getByRole('button',{name:'确认保存身份',exact:true}).click();
    const command=await page.evaluate(()=>(window as any).identityFixture.commands.findLast((row:any)=>row.command==='identities_save'));
    assert.equal(command.args.currentName,'rio.wt');
    await page.evaluate(()=>{const s=(window as any).identityFixture.state;s.workspace_id=s.records.find((r:any)=>r.name==='rio.wt').id;s.browser.bound_ids.push(s.workspace_id);});
    await page.getByRole('button',{name:'刷新',exact:true}).click();
    const saved=page.locator('.identity-card.current');assert.equal(await saved.getByRole('heading').innerText(),'rio.wt');
    assert.equal(await empty.getAttribute('class').then(c=>c?.includes('selected')),false,'saved browser-only identity must not highlight the empty card');
    assert.match(await saved.innerText(),/CLI 账号未识别/);
    await page.evaluate(()=>{const s=(window as any).identityFixture.state;s.active='empty';s.workspace_id=null;s.browser.enabled=false;s.browser_profile_present=false;});
    await page.getByRole('button',{name:'刷新',exact:true}).click();
    assert.equal(await page.getByRole('button',{name:'保存身份',exact:true}).count(),0,'genuinely empty unlinked state has nothing to save');
   } else {
    const current=page.locator('.identity-card.current');
    assert.match(await current.innerText(),/Claude 客户端运行中/);
    assert.match(await current.innerText(),/浏览器资料随身份切换/);
    await page.evaluate(()=>{const s=(window as any).identityFixture.state;s.running=false;s.processes=[];s.browser.bound_ids=[s.records[1].id];});
    await page.getByRole('button',{name:'刷新',exact:true}).click();
    assert.match(await current.innerText(),/Claude 客户端未运行/);
    assert.match(await page.locator('.identity-card').filter({has:page.getByRole('heading',{name:'Identity 2',exact:true})}).innerText(),/已关联浏览器资料/);
    await page.goto(base+'/tests/identities.html?panel=browser');
    await page.getByRole('heading',{name:'浏览器环境',exact:true}).waitFor();
    await page.evaluate(()=>{(window as any).identityFixture.browser.running=true;});
    await page.getByRole('button',{name:'刷新状态',exact:true}).click();
    assert.match(await page.locator('.browser-default').innerText(),/B 浏览器运行中.*浏览器资料随身份切换/);
   }
   console.log(`PASS ${name}`);
  } catch(error) {failures++;console.error(`FAIL ${name}`,error);}
  await context.close();
 }
} finally {
 await browser.close();await server.close();await rm(temporary,{recursive:true,force:true});
}
if(failures)process.exitCode=1;
