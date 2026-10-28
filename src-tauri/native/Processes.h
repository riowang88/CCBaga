/*
 * [INPUT]: 当前用户 libproc 快照和仅候选进程的 KERN_PROCARGS2。
 * [OUTPUT]: 带内核创建时间的进程记录、普通/系统签名克隆应用根、有界 argv、进程/参数失败诊断与共享退出窗口有界复核；未知仍阻断。
 * [POS]: Claude 与受管浏览器共用的 macOS 进程读取边界；不序列化 argv 或环境。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
#ifndef BAGA_PROCESSES_H
#define BAGA_PROCESSES_H
#import <Foundation/Foundation.h>
#include <libproc.h>
#include <sys/sysctl.h>
#include <sys/proc.h>
#include <signal.h>
#include <unistd.h>
#include <errno.h>
#ifndef BAGA_PROCESS_BUNDLE
static NSString *bagaProcessBundle(NSString *root) { return [NSBundle bundleWithPath:root].bundleIdentifier ?: @""; }
#define BAGA_PROCESS_BUNDLE bagaProcessBundle
#endif
static NSString *bagaProcessApp(NSString *path) {
    NSRange marker=[path rangeOfString:@".app/Contents/"];
    if (marker.location!=NSNotFound) return [path substringToIndex:marker.location+4];
    // macOS 的 code_sign_clone 可将主进程映射到 .app.bundle，仍需调用方核对包标识。
    marker=[path rangeOfString:@".app.bundle/Contents/"];
    return marker.location==NSNotFound ? nil : [path substringToIndex:marker.location+11];
}
static BOOL bagaProcessGone(pid_t pid) {
    if (kill(pid,0)!=0) return errno==ESRCH;
    // 僵尸仍占 PID，但已不能读写资料；libproc 对其可能返回 ESRCH。
    int mib[]={CTL_KERN,KERN_PROC,KERN_PROC_PID,pid}; struct kinfo_proc info={0}; size_t size=sizeof(info);
    return sysctl(mib,4,&info,&size,NULL,0)==0 && (size==0 || (size==sizeof(info) && info.kp_proc.p_stat==SZOMB));
}
// 各读取阶段共享退出确认：内核查询可先不可用，退出状态稍后才可见。
// 只等一次10ms；仍在运行、PID已复用或状态未知均保留错误，绝不重新接受另一生命周期。
static NSString *bagaProcessReadFailure(pid_t pid, NSString *stage, int result, int code, NSString *message) {
    if (bagaProcessGone(pid)) return nil;
    usleep(10000);
    if (bagaProcessGone(pid)) return nil;
    return [NSString stringWithFormat:@"%@ [stage=%@ pid=%d rc=%d errno=%d exit=unconfirmed]",message,stage,pid,result,code];
}
// 已更新/删除的可执行文件可能令 proc_pidpath 返回 ENOENT；使用内核保存的 exec 路径，绝不使用可伪造的 argv[0]。
static NSString *bagaSavedExecPath(pid_t pid) {
    int mib[]={CTL_KERN,KERN_PROCARGS2,pid}; size_t size=0;
    if (sysctl(mib,3,NULL,&size,NULL,0)!=0 || size>1048576 || size<=sizeof(int)) return nil;
    size_t allocated=size; char *raw=calloc(1,allocated); if (!raw) return nil;
    NSString *path=nil;
    if (sysctl(mib,3,raw,&size,NULL,0)==0 && size<=allocated && size>sizeof(int)) {
        const char *start=raw+sizeof(int); size_t bound=size-sizeof(int), length=strnlen(start,bound);
        if (length<bound) path=[[NSString alloc] initWithBytes:start length:length encoding:NSUTF8StringEncoding];
    }
    memset_s(raw,allocated,0,allocated); free(raw);
    return path.isAbsolutePath?path:nil;
}
static NSDictionary *bagaReadProcess(pid_t pid, NSString **error) {
    if (pid<=1) return nil;
    struct proc_bsdinfo info={0}, after={0};
    int infoResult=proc_pidinfo(pid,PROC_PIDTBSDINFO,0,&info,sizeof(info));int infoCode=infoResult<=0?errno:0;
    if (infoResult!=sizeof(info)) {
        *error=bagaProcessReadFailure(pid,@"identity-read",infoResult,infoCode,@"无法读取当前用户进程身份；未确认客户端退出。");
        return nil;
    }
    if (info.pbi_uid!=getuid() || info.pbi_status==SZOMB) return nil;
    char path[PROC_PIDPATHINFO_MAXSIZE]={0};
    NSString *executable=nil;
    int pathResult=proc_pidpath(pid,path,sizeof(path));int pathCode=pathResult<=0?errno:0;
    if (pathResult>0) executable=[[NSString alloc] initWithUTF8String:path];
    else if (pathCode==ENOENT) executable=bagaSavedExecPath(pid);
    if (!executable.length) {
        *error=bagaProcessReadFailure(pid,@"path-read",pathResult,pathCode,@"无法读取当前用户进程路径；未确认客户端退出。");
        return nil;
    }
    int afterResult=proc_pidinfo(pid,PROC_PIDTBSDINFO,0,&after,sizeof(after));int afterCode=afterResult<=0?errno:0;
    if (afterResult!=sizeof(after)) {
        *error=bagaProcessReadFailure(pid,@"identity-recheck",afterResult,afterCode,@"无法复核当前用户进程身份。");
        return nil;
    }
    if (after.pbi_uid!=info.pbi_uid || after.pbi_start_tvsec!=info.pbi_start_tvsec || after.pbi_start_tvusec!=info.pbi_start_tvusec) {
        *error=@"进程在核对期间已变化，请重试。"; return nil;
    }
    if (!executable.length) { *error=@"无法解析进程路径。"; return nil; }
    return @{@"pid":@(pid),@"parent":@(info.pbi_ppid),@"created":@(info.pbi_start_tvsec*1000000ULL+info.pbi_start_tvusec),@"executable":executable};
}
static BOOL bagaSameProcess(NSDictionary *a, NSDictionary *b) {
    return a && b && [a[@"pid"] isEqual:b[@"pid"]] && [a[@"created"] isEqual:b[@"created"]] && [a[@"executable"] isEqual:b[@"executable"]];
}
static NSArray<NSDictionary *> *bagaProcessList(NSString **error) {
    int size=proc_listpids(PROC_UID_ONLY,getuid(),NULL,0);
    if (size<=0 || size>16*1024*1024) { *error=@"无法枚举当前用户进程。"; return nil; }
    size+=64*sizeof(pid_t);
    pid_t *pids=calloc(1,size);
    if (!pids) { *error=@"无法分配进程核对缓冲区。"; return nil; }
    int used=proc_listpids(PROC_UID_ONLY,getuid(),pids,size);
    if (used<=0 || used>=size || used%sizeof(pid_t)!=0) { free(pids); *error=@"进程列表不完整，请重试。"; return nil; }
    NSMutableArray *result=[NSMutableArray array];
    for (int i=0;i<used/(int)sizeof(pid_t);i++) {
        NSDictionary *row=bagaReadProcess(pids[i],error);
        if (*error) { free(pids); return nil; }
        if (row) [result addObject:row];
    }
    free(pids); return result;
}
// 仅补充本地失败证据；退出未确认时不放行未完成的参数核验。
// errno 必须由调用处立即保存；退出复核可能改写线程 errno。绝不输出 argv/环境。
static NSString *bagaProcessArgsFailure(NSDictionary *row, NSString *stage, int result, int code, size_t size) {
    BOOL gone=bagaProcessGone([row[@"pid"] intValue]);
    return [NSString stringWithFormat:@"无法核对候选客户端参数；本次操作已停止。[stage=%@ pid=%@ executable=%@ rc=%d errno=%d bytes=%zu exit=%@]",
        stage,row[@"pid"],row[@"executable"],result,code,size,gone?@"confirmed":@"unconfirmed"];
}
// 与 bagaReadProcess 的退出语义一致：0 仅表示内核确认已退出，-1 仍为未知/冲突。
// EINVAL/ESRCH 可发生于进程退出窗口；最多复核一次，不重试整个身份操作。
static int bagaProcessArgsQuery(NSDictionary *row, void *buffer, size_t *size, NSString *stage, NSString **error) {
    int mib[]={CTL_KERN,KERN_PROCARGS2,[row[@"pid"] intValue]};
    size_t capacity=*size;
    for (int attempt=0; attempt<2; attempt++) {
        *size=capacity;
        int result=sysctl(mib,3,buffer,size,NULL,0); int code=result==0?0:errno;
        if (result==0) return 1;
        if (code==EINVAL || code==ESRCH) {
            NSString *checkError=nil;
            NSDictionary *current=bagaReadProcess([row[@"pid"] intValue],&checkError);
            if (!current && !checkError) return 0;
            if (current && !bagaSameProcess(row,current)) {
                *error=[bagaProcessArgsFailure(row,stage,result,code,*size) stringByAppendingString:@" 进程生命周期已变化。"];
                return -1;
            }
            // 退出途中可能先出现 args=EINVAL / pidinfo=ESRCH，稍后才确认退出。
            // 查询未知也走既有的一次复核；不把未知当退出，不重试明确的生命周期变化。
            if (attempt==0) { usleep(10000); continue; }
            if (checkError) {
                *error=[NSString stringWithFormat:@"%@ 生命周期复核失败：%@",bagaProcessArgsFailure(row,stage,result,code,*size),checkError];
                return -1;
            }
        }
        *error=bagaProcessArgsFailure(row,stage,result,code,*size);
        return -1;
    }
    return -1;
}
static NSArray<NSString *> *bagaProcessArgs(NSDictionary *row, NSString **error) {
    size_t size=0;
    if (bagaProcessArgsQuery(row,NULL,&size,@"args-size",error)!=1) return nil;
    if (size>1048576 || size<=sizeof(int)) { *error=bagaProcessArgsFailure(row,@"args-size",0,0,size); return nil; }
    size_t allocated=size; char *raw=calloc(1,allocated);
    if (!raw) { *error=@"无法分配客户端参数缓冲区。"; return nil; }
    NSMutableArray *args=[NSMutableArray array];
    if (bagaProcessArgsQuery(row,raw,&size,@"args-read",error)!=1) {
        memset_s(raw,allocated,0,allocated); free(raw); return nil;
    }
    if (size>allocated || size<=sizeof(int)) { *error=bagaProcessArgsFailure(row,@"args-read",0,0,size); }
    else {
        int argc=0; memcpy(&argc,raw,sizeof(argc)); char *cursor=raw+sizeof(argc), *end=raw+size;
        size_t length=strnlen(cursor,end-cursor);
        if (length==(size_t)(end-cursor) || argc<1 || argc>65536) *error=@"客户端参数格式无法确认。";
        else {
            cursor+=length;
            while (cursor<end && !*cursor) cursor++;
            for (int i=0;i<argc && !*error;i++) {
                if (cursor>=end) { *error=@"客户端参数不完整。"; break; }
                length=strnlen(cursor,end-cursor);
                if (length==(size_t)(end-cursor)) { *error=@"客户端参数未完整终止。"; break; }
                NSString *arg=[[NSString alloc] initWithBytes:cursor length:length encoding:NSUTF8StringEncoding];
                if (!arg) { *error=@"客户端参数编码无法确认。"; break; }
                [args addObject:arg]; cursor+=length+1;
            }
        }
    }
    memset_s(raw,allocated,0,allocated); free(raw);
    if (*error) return nil;
    NSDictionary *after=bagaReadProcess([row[@"pid"] intValue],error);
    if (!after && !*error) return nil;
    if (!bagaSameProcess(row,after)) { if (!*error) *error=@"候选客户端在参数核对期间已变化，请重试。"; return nil; }
    return args;
}
#endif
