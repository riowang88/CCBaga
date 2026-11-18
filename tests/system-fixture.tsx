/**
 * [INPUT]: 真实 SystemPanel、Tauri mockIPC 与虚构电脑快照（新加坡时区、zh-Hans-SG 首选语言、定位关闭）；13 项地区目录逐字取自 src-tauri/src/regions.rs。
 * [OUTPUT]: 电脑环境页可视/交互样本：system_read/get_regions/system_set_region/system_disable_location/browser_read/browser_rotate/browser_launch 只写入命令审计，不改本机；window.systemFixture 暴露 commands、state、browserRunning。
 * [POS]: 无真实系统修改、浏览器或网络的 GUI 回归样本，外壳与 main.tsx 相同。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import '../src/index.css';
import '../src/style.css';
import {createRoot} from 'react-dom/client';
import {mockIPC} from '@tauri-apps/api/mocks';
import {MotionRoot,TooltipProvider} from 'cromma-ui';
import {SystemPanel,type Region} from '../src/system';
import {setLocale} from '../src/i18n';
import {FixtureShell} from './fixture-shell';
const regions:Region[]=[{"id":"us-west","label":"美国 · 洛杉矶","country":"US","language":"en-US","locale":"en_US","timezone":"America/Los_Angeles"},{"id":"us-east","label":"美国 · 纽约","country":"US","language":"en-US","locale":"en_US","timezone":"America/New_York"},{"id":"ca","label":"加拿大 · 多伦多","country":"CA","language":"en-CA","locale":"en_CA","timezone":"America/Toronto"},{"id":"gb","label":"英国 · 伦敦","country":"GB","language":"en-GB","locale":"en_GB","timezone":"Europe/London"},{"id":"fr","label":"法国 · 巴黎","country":"FR","language":"fr","locale":"fr_FR","timezone":"Europe/Paris"},{"id":"de","label":"德国 · 柏林","country":"DE","language":"de","locale":"de_DE","timezone":"Europe/Berlin"},{"id":"jp","label":"日本 · 东京","country":"JP","language":"ja","locale":"ja_JP","timezone":"Asia/Tokyo"},{"id":"kr","label":"韩国 · 首尔","country":"KR","language":"ko","locale":"ko_KR","timezone":"Asia/Seoul"},{"id":"sg","label":"新加坡","country":"SG","language":"en-SG","locale":"en_SG","timezone":"Asia/Singapore"},{"id":"tw","label":"台湾 · 台北","country":"TW","language":"zh-Hant","locale":"zh_TW","timezone":"Asia/Taipei"},{"id":"hk","label":"香港","country":"HK","language":"zh-Hant","locale":"zh_HK","timezone":"Asia/Hong_Kong"},{"id":"cn","label":"中国 · 上海","country":"CN","language":"zh-Hans","locale":"zh_CN","timezone":"Asia/Shanghai"},{"id":"au","label":"澳大利亚 · 悉尼","country":"AU","language":"en-AU","locale":"en_AU","timezone":"Australia/Sydney"}];
const state={platform:'macos',timezone:'Asia/Singapore',timezones:['America/Los_Angeles','America/New_York','Asia/Singapore','Asia/Tokyo','Europe/London'],automatic_timezone:false,languages:['zh-Hans-SG'],locale:'zh_SG',location_enabled:false,accessibility:true};
const commands:{command:string;args:unknown}[]=[];
const fixture={commands,state,browserRunning:false,language:setLocale};
Object.assign(window,{systemFixture:fixture});
mockIPC((command,args)=>{
 if(command==='plugin:event|listen')return 1;
 if(command==='plugin:event|unlisten')return;
 if(command==='system_read')return structuredClone(state);
 if(command==='get_regions')return structuredClone(regions);
 commands.push({command,args:command==='browser_launch'?null:args});
 if(command==='system_set_region'){const r=regions.find(x=>x.id===(args as any).region)!;Object.assign(state,{timezone:r.timezone,languages:[r.language],locale:r.locale});return {state:structuredClone(state),message:'虚构：已设置地区'};}
 if(command==='system_disable_location'){state.location_enabled=false;return {state:structuredClone(state),message:'虚构：定位已关闭'};}
 if(command==='browser_read')return {running:fixture.browserRunning};
 if(command==='browser_rotate'){fixture.browserRunning=false;return {};}
 if(command==='browser_launch'){fixture.browserRunning=true;return {};}
 throw Error(`Unexpected fixture command: ${command}`);
});
setLocale('zh');
createRoot(document.getElementById('fixture')!).render(<MotionRoot><TooltipProvider><FixtureShell page="system"><SystemPanel/></FixtureShell></TooltipProvider></MotionRoot>);
