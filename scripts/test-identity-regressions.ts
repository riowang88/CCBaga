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
 for(const name of ['policy','selection','status-labels','browser-only','client-materials','controlled-login','single-client']) {
  const context=await browser.newContext();
  await context.route('**/*',route=>route.request().url().startsWith(base+'/')?route.continue():route.abort());
  const page=await context.newPage();
  page.setDefaultTimeout(3000);
  try {
   await page.goto(base+'/tests/identities.html');
   await page.getByText('fixture@example.invalid',{exact:true}).waitFor();
   if(name==='single-client') {
    await page.evaluate(()=>{const f=(window as any).identityFixture;f.state.browser.current_id=f.state.records[0].id;});
    await page.getByRole('button',{name:'刷新',exact:true}).click();
    await page.getByRole('button',{name:'保存 App 备份',exact:true}).click();
    const dialog=page.getByRole('alertdialog');
    assert.match(await dialog.innerText(),/Identity 1/);
    assert.match(await dialog.innerText(),/需要你本人核对/);
    const submit=dialog.getByRole('button',{name:'确认保存备份',exact:true});
    assert.equal(await submit.isDisabled(),true);
    await dialog.getByRole('checkbox',{name:'账号和组织已核对，App 与 CLI 已退出'}).check();
    await page.evaluate(()=>(window as any).identityFixture.setFailure('identities_associate'));
    await submit.click();await dialog.getByRole('alert').waitFor();assert.match(await dialog.innerText(),/fixture operation failed/);
    await dialog.getByRole('checkbox',{name:'账号和组织已核对，App 与 CLI 已退出'}).check();
    await submit.click();await dialog.waitFor({state:'hidden'});
    const commands=await page.evaluate(()=>(window as any).identityFixture.commands);
    const call=commands.filter((r:any)=>r.command==='identities_associate').at(-1);
    assert.equal(call.args.id,'00000000000000000000000000000001');assert.equal(call.args.client,'desktop');assert.equal(call.args.confirmed,true);
    assert.equal(commands.filter((r:any)=>/identities_(stop|save|switch)$/.test(r.command)).length,0,'single-client update does not invoke full save or stop');
   } else if(name==='controlled-login') {
    await page.getByRole('button',{name:'登录 CLI'}).click();
    const dialog=page.getByRole('dialog');
    const start=dialog.getByRole('button',{name:'打开浏览器登录',exact:true});
    assert.match(await dialog.innerText(),/Pro.*Max.*Team.*Enterprise/);
    assert.match(await dialog.innerText(),/免费账号/);
    assert.match(await dialog.innerText(),/Console.*API/);
    await dialog.getByText('查看运行程序（1）',{exact:true}).click();
    assert.match(await dialog.innerText(),/PID 4201/);
    assert.match(await dialog.innerText(),/VS Code/);
    await page.evaluate(()=>{const f=(window as any).identityFixture;f.state.processes=null;});
    await dialog.getByRole('button',{name:'刷新进程',exact:true}).click();
    assert.match(await dialog.innerText(),/进程状态未知/);
    await page.evaluate(()=>{const f=(window as any).identityFixture;f.state.processes=[];f.state.running=false;});
    await dialog.getByRole('button',{name:'刷新进程',exact:true}).click();
    assert.match(await dialog.innerText(),/未检测到 Claude 进程/);
    assert.equal(await start.isDisabled(),true);
    await dialog.getByRole('textbox',{name:'目标账号邮箱'}).fill('fixture@example.invalid');
    assert.equal(await start.isDisabled(),true);
    await dialog.getByRole('checkbox',{name:'B 浏览器和邮箱已核对，允许 CLI 保存登录信息'}).check();
    await page.evaluate(()=>(window as any).identityFixture.defer('identities_login'));
    await start.click();
    assert.equal(await start.isDisabled(),true);
    await dialog.getByRole('button',{name:'取消登录',exact:true}).click();
    await page.evaluate(()=>(window as any).identityFixture.release('identities_login','fixture login cancelled; credentials may have changed'));
    await dialog.getByRole('alert').waitFor();
    assert.match(await dialog.innerText(),/fixture login cancelled/);
    assert.equal(await page.evaluate(()=>(window as any).identityFixture.commands.filter((r:any)=>r.command==='identities_login_cancel').length),1);
    await dialog.getByRole('checkbox',{name:'B 浏览器和邮箱已核对，允许 CLI 保存登录信息'}).check();
    await start.click();
    await dialog.getByText('登录结果',{exact:true}).waitFor();
    assert.match(await dialog.innerText(),/再保存 CLI 备份/);
    assert.equal(await page.evaluate(()=>(window as any).identityFixture.commands.filter((r:any)=>/identities_(save|switch|stop)/.test(r.command)).length),0);
   } else if(name==='client-materials') {
    const panel=page.locator('.identity-client-status');
    await panel.getByText('查看本机数据',{exact:true}).click();
    assert.match(await panel.innerText(),/尚未确认/,'old IPC metadata remains unknown');
    assert.match(await panel.innerText(),/发现数据不代表已经登录/);
    await page.evaluate(()=>{const s=(window as any).identityFixture.state;s.current.clients={cli:'present',desktop:'absent'};s.browser_profile_present=true;});
    await page.getByRole('button',{name:'刷新',exact:true}).click();
    assert.match(await panel.innerText(),/CLI.*发现本地数据/s);
    assert.match(await panel.innerText(),/App.*未发现本地数据/s);
    assert.match(await panel.innerText(),/发现浏览器数据/);
    await panel.getByRole('button',{name:'登录 CLI'}).click();
    let dialog=page.getByRole('dialog');
    assert.match(await dialog.innerText(),/B 浏览器/);
    assert.match(await dialog.innerText(),/登录后请保存备份/);
    await dialog.getByRole('button',{name:'关闭',exact:true}).click();
    assert.equal(await page.evaluate(()=>(window as any).identityFixture.commands.filter((r:any)=>/login|switch|save/.test(r.command)).length),0,'reading guidance cannot launch login or save data');
    await page.locator('.identity-card').filter({has:page.getByRole('heading',{name:'Identity 2',exact:true})}).getByRole('button',{name:'切换',exact:true}).click();
    dialog=page.getByRole('dialog');
    assert.match(await dialog.innerText(),/操作完成前，请勿启动/);
    assert.match(await dialog.innerText(),/目标档案内资料/);
    assert.match(await dialog.innerText(),/尚未确认/,'legacy archive must not infer CLI/App data from account title');
    await dialog.getByRole('button',{name:'取消',exact:true}).click();
    await page.evaluate(()=>(window as any).identityFixture.language('en'));
    await page.getByText('Sign in and back up',{exact:true}).waitFor();
    assert.match(await panel.innerText(),/Data does not confirm sign-in/);
   } else if(name==='policy') {
    await page.getByText('切换账号时保持不变的内容',{exact:true}).click();
    await page.getByRole('checkbox',{name:'会话历史',exact:true}).check();
    await page.reload();
    await page.getByText('fixture@example.invalid',{exact:true}).waitFor();
    await page.getByText('切换账号时保持不变的内容',{exact:true}).click();
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
