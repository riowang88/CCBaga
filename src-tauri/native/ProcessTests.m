/*
 * [INPUT]: 合成进程列表、生命周期、原始 argv 与临时浏览器锁。
 * [OUTPUT]: 查询未知与参数诊断、确认退出/身份查询短暂不可用/有界重试/退出竞态、PID 复用、精确资料参数、主进程/helper 与未知锁回归。
 * [POS]: 替换 libproc/sysctl/kill 的隔离验证，不枚举或结束真实客户端、不读取账号。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
#import <AppKit/AppKit.h>
#import <Security/Security.h>
#import <LocalAuthentication/LocalAuthentication.h>
#import <CoreServices/CoreServices.h>
#import <CoreLocation/CoreLocation.h>
#import <SystemConfiguration/SystemConfiguration.h>
#include <libproc.h>
#include <sys/sysctl.h>
#include <sys/proc.h>
#include <signal.h>
#include <unistd.h>
#include <errno.h>
static NSArray<NSDictionary *> *rows;
static NSString *failure;
static int signalsSent;
static int infoCalls;
static int argsFaults;
static int exitProbeReads;
static int readCalls, goneChecks;
static BOOL readFaultPending;
static NSDictionary *rowFor(int pid) { for (NSDictionary *r in rows) if ([r[@"pid"] intValue] == pid) return r; return nil; }
static int fixtureListAll(void *buffer, int size) {
    if (!buffer) return (int)rows.count;
    int n = MIN((int)rows.count, size / (int)sizeof(pid_t));
    for (int i=0; i<n; i++) ((pid_t *)buffer)[i]=[rows[i][@"pid"] intValue];
    return n;
}
static int fixtureList(uint32_t type, uint32_t uid, void *buffer, int size) {
    NSCAssert(type==PROC_UID_ONLY && uid==getuid(), @"must enumerate only current user");
    if ([failure isEqual:@"list"]) return buffer?size:64;
    return fixtureListAll(buffer,size)*(int)sizeof(pid_t);
}
static int fixtureInfo(int pid, int flavor, uint64_t arg, void *buffer, int size) {
    NSDictionary *r=rowFor(pid);
    readCalls++;
    if (([failure hasPrefix:@"read-first-"] && readCalls==1) || ([failure hasPrefix:@"read-final-"] && readCalls==2)) {
        readFaultPending=YES;errno=ESRCH;return 0;
    }
    if (!r) { errno=ESRCH; return 0; }
    if ([r[@"zombie"] boolValue]) { errno=ESRCH; return 0; }
    if (([failure hasPrefix:@"args-exiting-"] || [failure isEqual:@"args-identity-recovers"] || [failure isEqual:@"args-reused-after-unknown"]) && argsFaults>0 && exitProbeReads++==0) { errno=ESRCH; return 0; }
    if ([failure isEqual:@"args-identity-unknown"] && argsFaults>0) { errno=EPERM; return 0; }
    if ([failure isEqual:@"info"]) { errno=EPERM; return 0; }
    struct proc_bsdinfo info={0}; info.pbi_pid=pid; info.pbi_uid=getuid();
    info.pbi_ppid=[r[@"parent"] intValue]; info.pbi_start_tvsec=[r[@"born"] unsignedLongLongValue]+([failure isEqual:@"reuse"] && ++infoCalls>1); info.pbi_start_tvusec=7;
    memcpy(buffer,&info,sizeof(info)); return sizeof(info);
}
static int fixturePath(int pid, void *buffer, uint32_t size) {
    if ([failure hasPrefix:@"read-path-"]) {readFaultPending=YES;errno=ESRCH;return 0;}
    if ([failure isEqual:@"path"]) { errno=EPERM; return 0; }
    if ([failure isEqual:@"unlinked"]) { errno=ENOENT; return 0; }
    const char *path=[rowFor(pid)[@"path"] UTF8String];
    if (!path) { errno=ESRCH; return 0; }
    strlcpy(buffer,path,size); return (int)strlen(path);
}
static int fixtureArgs(int *name, u_int count, void *out, size_t *size, void *newValue, size_t newSize) {
    if (count==4) {
        NSDictionary *r=rowFor(name[3]); struct kinfo_proc info={0};
        if (!r) { *size=0; return 0; }
        info.kp_proc.p_stat=[r[@"zombie"] boolValue]?SZOMB:SRUN;
        memcpy(out,&info,sizeof(info)); *size=sizeof(info); return 0;
    }
    if (([failure isEqual:@"args-exiting-size"] && !out) || ([failure isEqual:@"args-exiting-read"] && out)) {
        if (argsFaults++>0) rows=@[];
        errno=EINVAL; return -1;
    }
    if ([failure isEqual:@"args-identity-unknown"]) { argsFaults++; errno=EINVAL; return -1; }
    if ([failure isEqual:@"args-identity-recovers"] && !out && argsFaults++==0) { errno=EINVAL; return -1; }
    if ([failure isEqual:@"args-reused-after-unknown"] && !out) {
        if (argsFaults++==0) { errno=EINVAL; return -1; }
        NSMutableDictionary *changed=[rows[0] mutableCopy]; changed[@"born"]=@200; rows=@[changed];
    }
    if ([failure isEqual:@"args-transient"] && !out && argsFaults++==0) { errno=EINVAL; return -1; }
    if ([failure isEqual:@"args-read-transient"] && out && argsFaults++==0) { errno=EINVAL; return -1; }
    if ([failure isEqual:@"args-sticky-invalid"]) { errno=EINVAL; return -1; }
    if ([failure isEqual:@"args-reused"]) {
        NSMutableDictionary *changed=[rows[0] mutableCopy]; changed[@"born"]=@200; rows=@[changed]; errno=EINVAL; return -1;
    }
    if ([failure isEqual:@"args-exit-size"] || ([failure isEqual:@"args-exit-read"] && out)) {
        rows=@[]; errno=EINVAL; return -1;
    }
    if ([failure isEqual:@"args-read"] && out) { errno=EACCES; return -1; }
    if ([failure isEqual:@"args-empty"] && !out) { *size=0; errno=EPERM; return 0; }
    if ([failure isEqual:@"args"]) { errno=EPERM; return -1; }
    NSDictionary *r=rowFor(name[2]); if (!r) { errno=ESRCH; return -1; }
    NSArray *args=r[@"args"];
    int argc=(int)args.count; NSMutableData *data=[NSMutableData dataWithBytes:&argc length:sizeof(argc)];
    NSData *exe=[r[@"path"] dataUsingEncoding:NSUTF8StringEncoding]; [data appendData:exe]; [data appendBytes:"\0\0" length:2];
    for (NSString *arg in args) { [data appendData:[arg dataUsingEncoding:NSUTF8StringEncoding]]; [data appendBytes:"\0" length:1]; }
    if (!out) { *size=data.length; return 0; }
    if (*size<data.length) { errno=ENOMEM; return -1; }
    memcpy(out,data.bytes,data.length); *size=data.length; return 0;
}
static int fixtureKill(pid_t pid, int signal) {
    if (!signal && readFaultPending && ++goneChecks==2) {
        if ([failure hasSuffix:@"-exit"]) rows=@[];
        if ([failure hasSuffix:@"-reuse"]) {NSMutableDictionary *changed=[rows[0] mutableCopy];changed[@"born"]=@200;rows=@[changed];}
    }
    if (!rowFor(pid)) { errno=ESRCH; return -1; }
    if (signal) signalsSent++;
    return 0;
}
static NSString *fixtureBundle(NSString *root) {
    if ([root isEqual:@"/Applications/Claude.app"]) return @"com.anthropic.claudefordesktop";
    if ([root isEqual:@"/Applications/CCBaga-B.app"] || [root isEqual:@"/private/fixture/CCBaga-B.app.bundle"]) return @"ai.local.requirement-french.browser";
    return @"unrelated.app";
}
#define proc_listallpids fixtureListAll
#define proc_listpids fixtureList
#define proc_pidinfo fixtureInfo
#define proc_pidpath fixturePath
#define sysctl fixtureArgs
#define kill fixtureKill
#define BAGA_PROCESS_BUNDLE fixtureBundle
#include "Cleanup.m"
#include "Browser.m"
static id json(char *value) { if (!value) return nil; NSData *d=[[NSString stringWithUTF8String:value] dataUsingEncoding:NSUTF8StringEncoding]; free(value); return [NSJSONSerialization JSONObjectWithData:d options:0 error:nil]; }
static NSDictionary *process(int pid, int parent, NSString *path, NSArray *args) {
    return @{@"pid":@(pid),@"parent":@(parent),@"born":@100,@"path":path,@"args":args};
}
static void expectError(id value, NSString *reason) {
    NSCAssert([value isKindOfClass:NSDictionary.class] && value[@"error"], @"%@",reason);
}
int main(void) { @autoreleasepool {
    NSString *cli=@"/opt/node_modules/@anthropic-ai/claude-code/cli.js";
    NSString *main=@"/Applications/CCBaga-B.app/Contents/MacOS/Chromium";
    NSString *helper=@"/Applications/CCBaga-B.app/Contents/Frameworks/Chromium Framework.framework/Versions/1/Helpers/Chromium Helper.app/Contents/MacOS/Chromium Helper";
    NSString *profile=[NSTemporaryDirectory() stringByAppendingPathComponent:[@"CCBaga fixture " stringByAppendingString:NSUUID.UUID.UUIDString]];
    for (NSString *stage in @[@"first",@"path",@"final"]) {
        for (NSString *outcome in @[@"exit",@"unknown",@"reuse"]) {
            failure=[NSString stringWithFormat:@"read-%@-%@",stage,outcome];
            rows=@[process(42001,1,@"/usr/local/bin/node",@[@"node",cli])];
            readCalls=0;goneChecks=0;readFaultPending=NO;signalsSent=0;
            NSString *error=nil;NSDictionary *value=bagaReadProcess(42001,&error);
            if ([outcome isEqual:@"exit"]) NSCAssert(!value&&!error,@"confirmed exit after transient %@ read failure must not abort",stage);
            else {
                NSCAssert(!value&&error,@"%@ must reject %@",stage,outcome);
                NSCAssert([error containsString:@"pid=42001"] && [error containsString:@"errno=3"],@"read failure needs bounded diagnostic evidence");
            }
            NSCAssert(signalsSent==0,@"exit confirmation must not signal clients");
            readFaultPending=NO;
        }
    }
    for (NSString *fault in @[@"args",@"info",@"path",@"list",@"reuse"]) {
        rows=@[process(42001,1,@"/usr/local/bin/node",@[@"node",cli])]; failure=fault; infoCalls=0;
        expectError(json(rf_claude_processes()),@"unreadable candidate must not become an empty list");
    }
    for (NSString *fault in @[@"args",@"args-read",@"args-sticky-invalid",@"args-reused",@"args-identity-unknown",@"args-empty"]) {
        rows=@[process(42001,1,@"/usr/local/bin/node",@[@"node",@"private-fixture-token"])];
        failure=fault; signalsSent=0; argsFaults=0; exitProbeReads=0;
        id result=json(rf_claude_processes());
        expectError(result,@"diagnostics must not turn an unreadable or exited candidate into success");
        NSString *detail=result[@"error"];
        BOOL reading=[fault isEqual:@"args-read"];
        int expectedErrno=([fault isEqual:@"args-sticky-invalid"] || [fault isEqual:@"args-reused"] || [fault isEqual:@"args-identity-unknown"])?EINVAL:([fault isEqual:@"args-empty"]?0:(reading?EACCES:EPERM));
        for (NSString *field in @[@"pid=42001",@"executable=/usr/local/bin/node",
            reading?@"stage=args-read":@"stage=args-size",
            @"exit=unconfirmed",
            [NSString stringWithFormat:@"errno=%d",expectedErrno]]) {
            NSCAssert([detail containsString:field],@"missing diagnostic field %@: %@",field,detail);
        }
        if ([fault isEqual:@"args-empty"]) NSCAssert([detail containsString:@"rc=0"] && [detail containsString:@"bytes=0"],@"successful but invalid size must not inherit stale errno");
        NSCAssert(![detail containsString:@"private-fixture-token"],@"error diagnostics must not expose argv");
        NSCAssert(signalsSent==0,@"diagnostics are read-only");
    }
    for (NSString *fault in @[@"args-exit-size",@"args-exit-read",@"args-exiting-size",@"args-exiting-read"]) {
        rows=@[process(42001,1,@"/usr/local/bin/node",@[@"node",@"/tmp/unrelated.js"])]; failure=fault; argsFaults=0;exitProbeReads=0;
        id result=json(rf_claude_processes());
        NSCAssert([result isKindOfClass:NSArray.class] && [result count]==0,@"kernel-confirmed exit during argument query follows the existing exited-process rule");
        argsFaults=0;exitProbeReads=0;rows=@[process(42001,1,main,@[@"Chromium",@"--user-data-dir",profile])];
        id browser=json(rf_browser_running(profile.UTF8String));
        NSCAssert(!browser[@"error"] && ![browser[@"running"] boolValue],@"confirmed exited browser is not occupied");
    }
    for (NSString *fault in @[@"args-transient",@"args-read-transient",@"args-identity-recovers"]) {
        failure=fault;argsFaults=0;exitProbeReads=0;
        rows=@[process(42001,1,@"/usr/local/bin/node",@[@"node",cli])];
        id result=json(rf_claude_processes());
        NSCAssert([result isKindOfClass:NSArray.class] && [result count]==1,@"a readable same-lifecycle CLI must still be detected after bounded retry");
        argsFaults=0;exitProbeReads=0;rows=@[process(42001,1,@"/usr/local/bin/node",@[@"node",@"/tmp/unrelated.js"])];
        result=json(rf_claude_processes());
        NSCAssert([result isKindOfClass:NSArray.class] && [result count]==0,@"same-lifecycle unrelated runtime remains legal");
    }
    failure=@"args-reused-after-unknown";argsFaults=0;exitProbeReads=0;
    rows=@[process(42001,1,@"/usr/local/bin/node",@[@"node",cli])];
    expectError(json(rf_claude_processes()),@"successful argument retry must still reject a changed lifecycle");
    failure=@"";
    for (NSNumber *busy in @[@NO,@YES]) {
        NSMutableArray *browserRows=[NSMutableArray arrayWithObject:process(42009,1,@"/tmp/unrelated",@[@"unrelated"])];
        if (busy.boolValue) [browserRows addObject:process(42001,1,main,@[@"Chromium",@"--user-data-dir",profile])];
        rows=browserRows;
        id value=json(rf_browser_running(profile.UTF8String));
        NSCAssert([value isKindOfClass:NSDictionary.class] && !value[@"error"],@"browser occupancy fixture must be readable");
        id running=value[@"running"];
        NSCAssert(running && CFGetTypeID((__bridge CFTypeRef)running)==CFBooleanGetTypeID(),@"browser bridge must emit a JSON boolean, not numeric 0/1");
        NSCAssert([running boolValue]==busy.boolValue,@"strict wire type must preserve active and idle states");
    }
    failure=@"";
    rows=@[process(42001,1,@"/usr/local/bin/node",@[@"node",@"--require",@"/tmp/preload.js",cli,@"private-fixture-token"])];
    NSArray *state=json(rf_claude_processes());
    NSCAssert(state.count==1 && [state[0][@"created"] unsignedLongLongValue]==100000007,@"snapshot must carry kernel creation time");
    NSCAssert(![state.description containsString:@"private-fixture-token"],@"argv must not escape the native boundary");
    signalsSent=0;
    json(rf_signal_process(42001,100000008,"/usr/local/bin/node",15));
    json(rf_signal_process(42001,100000007,"/wrong/path",15));
    NSCAssert(signalsSent==0,@"PID reuse or changed executable must not be signaled");
    json(rf_signal_process(42001,100000007,"/usr/local/bin/node",15));
    NSCAssert(signalsSent==1,@"the captured lifecycle can be signaled");
    failure=@"unlinked";
    state=json(rf_claude_processes());
    NSCAssert(state.count==1,@"unlinked executable uses saved kernel exec path");
    failure=@"";
    NSMutableDictionary *zombie=[rows[0] mutableCopy]; zombie[@"zombie"]=@YES; rows=@[zombie];
    NSCAssert([json(rf_claude_processes()) count]==0,@"kernel-confirmed zombie is exited");
    rows=@[process(42001,1,@"/usr/local/bin/node",@[@"node",@"/tmp/unrelated.js",cli]), process(42003,1,@"/tmp/claude",@[@"claude"])];
    NSCAssert([json(rf_claude_processes()) count]==0,@"unrelated script or unsigned same-name executable is not Claude");
    NSString *cloneMain=@"/private/fixture/CCBaga-B.app.bundle/Contents/MacOS/Chromium";
    rows=@[process(42001,1,cloneMain,@[@"Chromium",@"--user-data-dir",profile])];
    NSCAssert([json(rf_browser_running(profile.UTF8String))[@"running"] boolValue],@"macOS signed app clone must remain visible to profile occupancy checks");
    rows=@[process(42001,1,@"/private/fixture/unrelated.app.bundle/Contents/MacOS/Chromium",@[@"Chromium",@"--user-data-dir",profile])];
    NSCAssert(![json(rf_browser_running(profile.UTF8String))[@"running"] boolValue],@"clone-shaped path without the known bundle ID is not a managed browser");
    rows=@[process(42002,1,helper,@[@"Chromium Helper",@"--type=utility",[@"--user-data-dir=" stringByAppendingString:profile]])];
    NSCAssert([json(rf_browser_running(profile.UTF8String))[@"running"] boolValue],@"helper remains busy without lock or main");
    rows=@[process(42001,1,main,@[@"Chromium",@"--user-data-dir",profile]),process(42002,42001,helper,@[@"Chromium Helper",@"--type=renderer"])];
    NSCAssert([json(rf_browser_running(profile.UTF8String))[@"running"] boolValue],@"helper inherits exact parent profile");
    NSString *other=[profile stringByAppendingString:@"-other"];
    NSCAssert(![json(rf_browser_running(other.UTF8String))[@"running"] boolValue],@"another exact profile is not blocked");
    rows=@[process(42002,42001,helper,@[@"Chromium Helper",@"--type=renderer"])];
    expectError(json(rf_browser_running(profile.UTF8String)),@"orphan without profile remains unknown");
    rows=@[process(42001,1,main,@[@"Chromium",@"--user-data-dir",profile,@"--user-data-dir",other])];
    expectError(json(rf_browser_running(profile.UTF8String)),@"conflicting profile arguments remain unknown");
    rows=@[process(42001,1,@"/tmp/unrelated",@[@"unrelated"])];
    NSFileManager *fm=NSFileManager.defaultManager;
    NSCAssert([fm createDirectoryAtPath:profile withIntermediateDirectories:YES attributes:nil error:nil],@"fixture directory");
    NSString *lock=[profile stringByAppendingPathComponent:@"SingletonLock"];
    [@"unknown-lock" writeToFile:lock atomically:YES encoding:NSUTF8StringEncoding error:nil];
    expectError(json(rf_browser_running(profile.UTF8String)),@"unknown lock is not empty");
    NSCAssert([fm fileExistsAtPath:lock],@"unknown lock must be preserved");
    [fm removeItemAtPath:lock error:nil]; char hostname[256]={0}; gethostname(hostname,sizeof(hostname));
    NSCAssert(symlink([[NSString stringWithFormat:@"%s-49999",hostname] UTF8String],lock.fileSystemRepresentation)==0,@"fixture stale lock");
    NSCAssert(![json(rf_browser_running(profile.UTF8String))[@"running"] boolValue],@"confirmed stale lock does not invent an active client");
    NSCAssert([fm destinationOfSymbolicLinkAtPath:lock error:nil]!=nil,@"query must not remove stale lock");
    [fm removeItemAtPath:profile error:nil];
    puts("Process fixtures passed: query failures, lifecycle/PID reuse, metadata privacy, deleted executable, zombie, unrelated processes, exact profiles, helper ancestry/orphans and preserved locks.");
} return 0; }
