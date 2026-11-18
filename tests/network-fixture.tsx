/**
 * [INPUT]: 真实网络页、Tauri mockIPC 与虚构的诊断/浏览器状态（四个固定域名含 Cloudflare 浏览器质询可达、本机出口、一致性核验、DNS 最近一轮）。
 * [OUTPUT]: networkFixture.demo() 填入真实量级的视觉核对数据；可控的成功/部分失败/命令异常/缺失结果，检测期间由测试显式释放；B 运行状态、DNS 结果与命令审计可由测试切换。
 * [POS]: 无真实连接、密钥或外部导航的 GUI 回归样本。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import '../src/index.css';
import '../src/style.css';
import {createRoot} from 'react-dom/client';
import {mockIPC} from '@tauri-apps/api/mocks';
import {MotionRoot,TooltipProvider} from 'cromma-ui';
import {VpsPanel} from '../src/vps';
import {setLocale} from '../src/i18n';
import type {BrowserState} from '../src/browser-model';
import {FixtureShell} from './fixture-shell';
let mode='success';
let release=()=>{};
const commands:{command:string;args:unknown}[]=[];
const browser:BrowserState={ui_language:'zh',exit_services:[],region:{id:'us',label:'美国',country:'US',language:'en-US',locale:'en_US',timezone:'America/New_York'},config:{seed:123,proxy:'',follow_machine:true,language:'en-US',timezone:'America/New_York',native_graphics:true},machine:{platform:'macos',version:'fixture',cores:8,timezone:'America/New_York',languages:['en-US'],locale:'en_US',locale_country:'US',location_enabled:false,timezones:['America/New_York']},engine_version:'fixture',engine_ready:true,running:false,profile:'/fixture/profiles/current',identity_linked:true,cache:[],cache_error:'',report:null,report_received_at:null,machine_exit:null,dns_last:null,
 checks:[{title:'时区一致',status:'ok',detail:'本机：America/New_York；浏览器：待测；出口：待测'},{title:'目标语言与系统区域',status:'mismatch',detail:'fixture mismatch'},{title:'WebRTC / UDP 出口',status:'pending',detail:'fixture pending'}]};

// 视觉核对用：填入与真实机器同量级的数据（12 项一致性、6 个解析器、长运营商名），再跑一次检测。
function demo(){
 const titles=['目标地区网络出口','浏览器与机器出口','时区一致','目标语言与系统区域','真实设备与 CPU','浏览器版本','地理位置关闭','真实图形与字体','WebRTC / UDP 出口','屏幕','字体','音频'];
 browser.running=true;browser.report_received_at=Date.now();
 browser.dns_last={observation:{observed_at:Date.now(),resolvers:([['183.232.250.151','CN','China','','China Mobile'],['162.158.92.88','US','United States','','Cloudflare'],['47.106.126.96','CN','China','Shenzhen','Hangzhou Alibaba Advertising'],['47.106.96.109','CN','China','Shenzhen','Hangzhou Alibaba Advertising'],['104.22.225.86','SG','Singapore','Singapore','Cloudflare'],['104.22.223.86','SG','Singapore','Singapore','Cloudflare']] as const).map(([ip,country_code,country,city,isp])=>({ip,country_code,country,city,isp}))},error:''} as any;
 browser.checks=titles.map((title,i)=>({title,status:i===7?'pending':i===3?'mismatch':'ok',detail:i%2?'上次检测的机器出口：139.162.41.130':'目标：新加坡；实际：SG · 139.162.41.130 · Singapore · Akamai Technologies, Inc.'}));
 demoExit=true;window.dispatchEvent(new Event('focus'));
}
let demoExit=false;
Object.assign(window,{networkFixture:{demo,commands,browser,setMode:(value:string)=>{mode=value;},finish:()=>release(),language:setLocale,refresh:()=>window.dispatchEvent(new Event('focus'))}});
mockIPC((command,args)=>{
 if(command==='plugin:event|listen')return 1;
 if(command==='plugin:event|unlisten')return;
 if(command==='browser_read')return structuredClone(browser);
 if(command==='browser_check'){commands.push({command,args});browser.machine_exit=demoExit?{ip:'139.162.41.130',country:'SG',city:'Singapore',timezone:'Asia/Singapore',isp:'Akamai Technologies, Inc. (Linode Singapore Datacenter)'}:{ip:'198.51.100.24',country:'US',city:'Fixture City',timezone:'America/New_York',isp:'Fixture ISP'};return structuredClone(browser);}
 if(command==='browser_launch'||command==='browser_open_report'){commands.push({command,args:command==='browser_launch'?{webrtc:(args as any)?.webrtc??null}:null});if(command==='browser_launch'){browser.running=true;return structuredClone(browser);}return;}
 if(command!=='network_diagnose_domains')throw Error(`Unexpected fixture command: ${command}`);
 return new Promise((resolve,reject)=>{release=()=>{
  if(mode==='error')return reject('fixture connection interrupted');
  if(mode==='empty')return resolve([]);
  if(mode==='invalid')return resolve(null);
  resolve(['claude.ai','api.anthropic.com','platform.claude.com','downloads.claude.ai'].map((domain,n)=>({domain,ok:mode!=='partial'||n!==1,status_code:mode==='partial'&&n===1?403:n===0?403:200,latency_ms:35+n,ip:'203.0.113.1',message:mode==='partial'&&n===1?'受到安全拦截或限制':n===0?'服务可达，需浏览器验证':'连接正常'})));
 };});
});
setLocale('zh');
createRoot(document.getElementById('fixture')!).render(<MotionRoot><TooltipProvider><FixtureShell page="vps"><VpsPanel/></FixtureShell></TooltipProvider></MotionRoot>);
