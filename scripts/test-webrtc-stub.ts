/**
 * [INPUT]: Playwright BrowserContext。
 * [OUTPUT]: stubWebRtc 用 init script 替换 RTCPeerConnection，按 window.__webrtcFixture 发出虚构 srflx 候选或模拟禁用/无候选，并记录收到的 iceServers。
 * [POS]: 测试专用；路由拦截管不住 UDP，伴生页测试必须用它防止真实 STUN 外连。不进入生产构建。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import type {BrowserContext} from 'playwright-core';
export async function stubWebRtc(context:BrowserContext) {
 await context.addInitScript(()=>{
  const w=window as any;
  w.__webrtcFixture={mode:'match',ip:'203.0.113.1',servers:[] as unknown[]};
  class FakePeer {
   onicecandidate:((event:{candidate:{candidate:string}|null})=>void)|null=null;
   onicegatheringstatechange:(()=>void)|null=null;
   iceGatheringState='new';localDescription=null;
   constructor(config:RTCConfiguration){
    if(w.__webrtcFixture.mode==='disabled')throw new Error('WebRTC disabled by fixture');
    w.__webrtcFixture.servers.push(config.iceServers);
   }
   createDataChannel(){return {};}
   async createOffer(){return {type:'offer',sdp:''};}
   async setLocalDescription(){
    const {mode,ip}=w.__webrtcFixture;
    setTimeout(()=>{
     if(mode!=='none')this.onicecandidate?.({candidate:{candidate:`candidate:1 1 udp 1677729535 ${mode==='mismatch'?'198.51.100.7':ip} 50000 typ srflx raddr 0.0.0.0 rport 0`}});
     this.onicecandidate?.({candidate:null});
    },10);
   }
   close(){}
  }
  w.RTCPeerConnection=FakePeer;
 });
}
