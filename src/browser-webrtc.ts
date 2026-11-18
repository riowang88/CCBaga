/**
 * [INPUT]: B 浏览器内 RTCPeerConnection 的 ICE 候选（固定公共 STUN）与同一轮 HTTP 出口 IP。
 * [OUTPUT]: STUN_SERVERS 常量、parseCandidate/udpExitIps/classifyWebRtc 纯函数与 probeWebRtc 采集；四态 match/mismatch/unknown/disabled。
 * [POS]: 伴生检测页唯一的 WebRTC 边界；不使用 TURN、不发送凭据、不接受外部传入的服务器地址；未知与禁用都不算通过，不并入 12 项基础核验。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
export const STUN_SERVERS=['stun:stun.l.google.com:19302','stun:stun.cloudflare.com:3478'] as const;
export const WEBRTC_TIMEOUT_MS=5000;
export type WebRtcStatus='match'|'mismatch'|'unknown'|'disabled';
export type WebRtcResult={status:WebRtcStatus;udp_ips:string[];error:string};
export type Candidate={type:string;protocol:string;ip:string};

function validIp(ip:string):boolean {
  if(ip.includes(':'))return /^[0-9A-Fa-f:.]+$/.test(ip)&&ip.split(':').length>=3&&ip.length<=45;
  return ip.split('.').length===4&&ip.split('.').every(x=>/^(0|[1-9]\d{0,2})$/.test(x)&&Number(x)<=255);
}
const normalize=(ip:string)=>ip.includes(':')?ip.toLowerCase():ip;

/** 解析一条 SDP candidate 行（可带 "a=" 前缀）；mDNS（.local）与非法地址返回 null。 */
export function parseCandidate(line:string):Candidate|null {
  const text=line.trim().replace(/^a=/,'');
  const parts=text.replace(/^candidate:/,'').split(/\s+/);
  if(!/^candidate:/.test(text)||parts.length<8)return null;
  const protocol=parts[2].toLowerCase(), ip=parts[4], typeIndex=parts.indexOf('typ');
  if(typeIndex<0||!parts[typeIndex+1]||!validIp(ip))return null;
  return {type:parts[typeIndex+1].toLowerCase(),protocol,ip:normalize(ip)};
}

/** 只把 UDP srflx 候选当作 STUN 看到的出口；host/mDNS/relay 不计入。去重、最多 8 个。 */
export function udpExitIps(lines:readonly string[]):string[] {
  const out:string[]=[];
  for(const line of lines){
    const candidate=parseCandidate(line);
    if(candidate?.type==='srflx'&&candidate.protocol==='udp'&&!out.includes(candidate.ip))out.push(candidate.ip);
    if(out.length>=8)break;
  }
  return out;
}

/** 与同一轮 HTTP 出口比对。任一 UDP 出口不同（含不同地址族）即「可能泄露」；缺 HTTP 出口或无候选为未知。 */
export function classifyWebRtc({disabled,udpIps,httpIp,error=''}:{disabled:boolean;udpIps:readonly string[];httpIp:string|null;error?:string}):WebRtcResult {
  const udp_ips=[...udpIps];
  if(disabled)return {status:'disabled',udp_ips:[],error:error||'WebRTC 已被浏览器策略禁用。'};
  if(!udp_ips.length)return {status:'unknown',udp_ips,error:error||'未取到 STUN 候选（超时、UDP 被拦或浏览器策略限制）。'};
  if(!httpIp)return {status:'unknown',udp_ips,error:'HTTP 出口未取得，无法比对 UDP 出口。'};
  const http=normalize(httpIp);
  return {status:udp_ips.every(ip=>ip===http)?'match':'mismatch',udp_ips,error:''};
}

type PeerFactory=(config:RTCConfiguration)=>RTCPeerConnection;
const defaultPeer:PeerFactory=config=>new RTCPeerConnection(config);

/** 在当前浏览器收集候选；超时或取消即停止并关闭连接。返回原始候选行，比对交给 classifyWebRtc。 */
export async function collectCandidates(signal:AbortSignal,{timeout=WEBRTC_TIMEOUT_MS,peer=defaultPeer}:{timeout?:number;peer?:PeerFactory}={}):Promise<{disabled:boolean;lines:string[];error:string}> {
  signal.throwIfAborted();
  if(peer===defaultPeer&&typeof RTCPeerConnection==='undefined')return {disabled:true,lines:[],error:''};
  let connection:RTCPeerConnection;
  try{connection=peer({iceServers:STUN_SERVERS.map(urls=>({urls})),iceCandidatePoolSize:0});}
  catch{return {disabled:true,lines:[],error:''};}
  const lines:string[]=[];
  try{
    await new Promise<void>((resolve,reject)=>{
      const done=()=>{clearTimeout(timer);signal.removeEventListener('abort',stop);resolve();};
      const stop=()=>{clearTimeout(timer);reject(signal.reason);};
      const timer=setTimeout(done,timeout);
      signal.addEventListener('abort',stop,{once:true});
      connection.onicecandidate=event=>{if(event.candidate?.candidate)lines.push(event.candidate.candidate);else done();};
      connection.onicegatheringstatechange=()=>{if(connection.iceGatheringState==='complete')done();};
      connection.createDataChannel('ccbaga-probe');
      connection.createOffer().then(offer=>connection.setLocalDescription(offer)).catch(error=>{clearTimeout(timer);signal.removeEventListener('abort',stop);reject(error);});
    });
    for(const line of connection.localDescription?.sdp.split(/\r?\n/)||[])if(line.startsWith('a=candidate:'))lines.push(line);
    return {disabled:false,lines,error:''};
  }catch(error){
    if(signal.aborted)throw error;
    return {disabled:false,lines,error:error instanceof Error?error.message:String(error)};
  }finally{connection.onicecandidate=null;connection.onicegatheringstatechange=null;connection.close();}
}
