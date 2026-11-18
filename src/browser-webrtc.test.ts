/**
 * [INPUT]: 虚构 SDP 候选行、伪 RTCPeerConnection 与 HTTP 出口。
 * [OUTPUT]: 候选解析、srflx 过滤、四态判定、超时/取消/禁用的离线回归。
 * [POS]: 不访问网络、不创建真实 WebRTC 连接。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import {test,expect} from 'bun:test';
import {STUN_SERVERS,classifyWebRtc,collectCandidates,parseCandidate,udpExitIps} from './browser-webrtc';
const srflx='candidate:842163049 1 udp 1677729535 203.0.113.9 51234 typ srflx raddr 192.168.1.5 rport 51234 generation 0';
test('固定 STUN 常量，不含 TURN 或凭据',()=>{
  expect([...STUN_SERVERS]).toEqual(['stun:stun.l.google.com:19302','stun:stun.cloudflare.com:3478']);
  expect(STUN_SERVERS.some(url=>/turn|@/.test(url))).toBe(false);
});
test('解析 srflx/host/mDNS/IPv6 与非法行',()=>{
  expect(parseCandidate(srflx)).toEqual({type:'srflx',protocol:'udp',ip:'203.0.113.9'});
  expect(parseCandidate('a='+srflx)?.ip).toBe('203.0.113.9');
  expect(parseCandidate('candidate:1 1 udp 2122260223 4f1b2c3d-aaaa.local 50000 typ host generation 0')).toBeNull();
  expect(parseCandidate('candidate:2 1 udp 1686052607 2001:DB8::1 50000 typ srflx raddr :: rport 0')?.ip).toBe('2001:db8::1');
  expect(parseCandidate('candidate:3 1 udp 1 999.1.1.1 1 typ srflx')).toBeNull();
  expect(parseCandidate('not a candidate')).toBeNull();
});
test('只取 UDP srflx，去重并限 8 个',()=>{
  const lines=[srflx,srflx,'candidate:4 1 udp 2122260223 192.168.1.5 50000 typ host','candidate:5 1 tcp 1518280447 198.51.100.7 9 typ srflx tcptype active','candidate:6 1 udp 41885439 198.51.100.8 3478 typ relay'];
  expect(udpExitIps(lines)).toEqual(['203.0.113.9']);
  expect(udpExitIps(Array.from({length:12},(_,n)=>`candidate:${n} 1 udp 1 198.51.100.${n} 1 typ srflx`)).length).toBe(8);
});
test('四态判定：一致/不一致/未知/禁用，未知不算通过',()=>{
  expect(classifyWebRtc({disabled:false,udpIps:['203.0.113.9'],httpIp:'203.0.113.9'}).status).toBe('match');
  const leak=classifyWebRtc({disabled:false,udpIps:['203.0.113.9','198.51.100.7'],httpIp:'203.0.113.9'});
  expect(leak.status).toBe('mismatch');expect(leak.udp_ips).toEqual(['203.0.113.9','198.51.100.7']);
  expect(classifyWebRtc({disabled:false,udpIps:['2001:db8::1'],httpIp:'203.0.113.9'}).status).toBe('mismatch');
  expect(classifyWebRtc({disabled:false,udpIps:['2001:db8::1'],httpIp:'2001:DB8::1'}).status).toBe('match');
  const none=classifyWebRtc({disabled:false,udpIps:[],httpIp:'203.0.113.9'});
  expect(none.status).toBe('unknown');expect(none.error).toMatch(/未取到 STUN 候选/);
  expect(classifyWebRtc({disabled:false,udpIps:['203.0.113.9'],httpIp:null}).status).toBe('unknown');
  expect(classifyWebRtc({disabled:true,udpIps:['203.0.113.9'],httpIp:'203.0.113.9'})).toEqual({status:'disabled',udp_ips:[],error:'WebRTC 已被浏览器策略禁用。'});
});
class FakePeer {
  onicecandidate:((event:{candidate:{candidate:string}|null})=>void)|null=null;
  onicegatheringstatechange:(()=>void)|null=null;
  iceGatheringState='new';localDescription:{sdp:string}|null=null;closed=false;
  constructor(public config:RTCConfiguration,private emit:string[]|null){}
  createDataChannel(){return {};}
  async createOffer(){return {type:'offer',sdp:''};}
  async setLocalDescription(){if(!this.emit)return;queueMicrotask(()=>{for(const c of this.emit!)this.onicecandidate?.({candidate:{candidate:c}});this.onicecandidate?.({candidate:null});});}
  close(){this.closed=true;}
}
test('采集：使用固定服务器、结束后关闭；超时返回空；构造失败视为禁用；取消抛出',async()=>{
  let made:FakePeer|null=null;
  const result=await collectCandidates(new AbortController().signal,{peer:config=>(made=new FakePeer(config,[srflx])) as unknown as RTCPeerConnection});
  expect(result).toEqual({disabled:false,lines:[srflx],error:''});
  expect(made!.config.iceServers).toEqual(STUN_SERVERS.map(urls=>({urls})));expect(made!.closed).toBe(true);
  const slow=await collectCandidates(new AbortController().signal,{timeout:20,peer:config=>new FakePeer(config,null) as unknown as RTCPeerConnection});
  expect(slow.lines).toEqual([]);expect(slow.disabled).toBe(false);
  expect((await collectCandidates(new AbortController().signal,{peer:()=>{throw new Error('blocked');}})).disabled).toBe(true);
  const controller=new AbortController();
  const pending=collectCandidates(controller.signal,{timeout:1000,peer:config=>new FakePeer(config,null) as unknown as RTCPeerConnection});
  controller.abort(new Error('cancelled'));
  await expect(pending).rejects.toThrow('cancelled');
});
