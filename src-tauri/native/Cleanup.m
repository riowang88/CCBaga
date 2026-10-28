/*
 * [INPUT]: 当前用户进程的可执行路径/Node 启动脚本、Claude 应用标识与 Security 钥匙串接口。
 * [OUTPUT]: 带内核创建时间的 Claude Desktop/Code 进程快照、候选查询未知/已确认退出区分与按生命周期复核的结束、脱敏凭据发现及原生单条直接删除。
 * [POS]: 清理的 macOS 边界；只读取凭据属性，扫描逐查询禁止交互，删除允许系统授权，不读取密码、不保留恢复副本。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
#import <AppKit/AppKit.h>
#import <Security/Security.h>
#import <LocalAuthentication/LocalAuthentication.h>
#include <libproc.h>
#include <sys/sysctl.h>
#include <unistd.h>

static char *cleanResponse(NSDictionary *data) {
    NSData *json = [NSJSONSerialization dataWithJSONObject:data options:0 error:NULL];
    return strdup([[NSString alloc] initWithData:json encoding:NSUTF8StringEncoding].UTF8String);
}
static char *credentialError(OSStatus status, NSString *stage);
#include "Processes.h"
// Node 参数只用于确定执行入口，参数不可读时必须返回未知。
static BOOL claudeNode(NSArray<NSString *> *args, NSString **error) {
    NSSet *takesValue=[NSSet setWithArray:@[@"-r",@"--require",@"--import",@"--loader",@"--experimental-loader",@"--conditions",@"-C",@"--inspect-port",@"--icu-data-dir",@"--openssl-config",@"--title"]];
    for (NSUInteger i=1;i<args.count;i++) {
        NSString *arg=args[i];
        if ([arg isEqual:@"-e"] || [arg isEqual:@"-p"] || [arg isEqual:@"--eval"] || [arg hasPrefix:@"--eval="] || [arg isEqual:@"--print"] || [arg hasPrefix:@"--print="]) return NO;
        if ([takesValue containsObject:arg]) { if (++i>=args.count) *error=@"Node 启动参数不完整。"; continue; }
        if ([arg isEqual:@"--"]) { if (++i>=args.count) return NO; arg=args[i]; }
        else if ([arg hasPrefix:@"-"]) continue;
        return [arg hasSuffix:@"/node_modules/@anthropic-ai/claude-code/cli.js"];
    }
    return NO;
}
static BOOL nativeClaude(NSString *path) {
    NSString *versions=[NSHomeDirectory() stringByAppendingPathComponent:@".local/share/claude/versions"];
    NSArray *parts=[path.lastPathComponent componentsSeparatedByString:@"."];
    if ([path.stringByDeletingLastPathComponent isEqual:versions] && parts.count==3) {
        BOOL numeric=YES;
        for (NSString *part in parts) {
            if (!part.length) numeric=NO;
            for (NSUInteger i=0;i<part.length;i++) if ([part characterAtIndex:i]<'0' || [part characterAtIndex:i]>'9') numeric=NO;
        }
        if (numeric) return YES;
    }
    if (![@[@"claude",@"claude.exe",@"claude-code",@"claude-code-vm"] containsObject:path.lastPathComponent]) return NO;
    // 自定义位置只接受官方签名标识，不能凭同名进程结束无关程序。
    SecStaticCodeRef code=NULL; CFDictionaryRef raw=NULL; BOOL match=NO;
    if (SecStaticCodeCreateWithPath((__bridge CFURLRef)[NSURL fileURLWithPath:path],kSecCSDefaultFlags,&code)==errSecSuccess) {
        if (SecCodeCopySigningInformation(code,kSecCSSigningInformation,&raw)==errSecSuccess) {
            NSDictionary *info=CFBridgingRelease(raw);
            match=[info[(__bridge id)kSecCodeInfoIdentifier] isEqual:@"com.anthropic.claude-code"] && [info[(__bridge id)kSecCodeInfoTeamIdentifier] isEqual:@"Q6L2SF6YDW"];
        }
        CFRelease(code);
    }
    return match;
}
char *rf_claude_processes(void) {
    NSString *error=nil; NSArray *rows=bagaProcessList(&error);
    if (error) return cleanResponse(@{@"error":error});
    NSMutableArray *matches=[NSMutableArray array];
    for (NSDictionary *row in rows) {
        NSString *path=row[@"executable"], *name=path.lastPathComponent;
        NSString *app=bagaProcessApp(path), *bundle=app?BAGA_PROCESS_BUNDLE(app):@"";
        BOOL desktop=[bundle isEqual:@"com.anthropic.claudefordesktop"] || [bundle hasPrefix:@"com.anthropic.claudefordesktop."];
        BOOL cli=nativeClaude(path);
        if ([@[@"node",@"nodejs",@"bun"] containsObject:name]) {
            NSArray *args=bagaProcessArgs(row,&error);
            if (error) return cleanResponse(@{@"error":error});
            if (!args) continue; // 查询期间已由内核确认退出；未知与 PID 复用仍返回错误。
            cli=claudeNode(args,&error);
            if (error) return cleanResponse(@{@"error":error});
        }
        if (desktop || cli) {
            NSMutableDictionary *result=[row mutableCopy];
            result[@"name"]=cli?@"Claude Code":([name isEqual:@"Claude"]?@"Claude Desktop":name);
            [matches addObject:result];
        }
    }
    NSData *json=[NSJSONSerialization dataWithJSONObject:matches options:0 error:NULL];
    return strdup([[NSString alloc] initWithData:json encoding:NSUTF8StringEncoding].UTF8String);
}
char *rf_process_identity(int pid) {
    NSString *error=nil; NSDictionary *row=bagaReadProcess(pid,&error);
    return cleanResponse(error?@{@"error":error}:@{@"process":row?:NSNull.null});
}
char *rf_signal_process(int pid, unsigned long long created, const char *executable, int signal) {
    NSString *error=nil; NSDictionary *row=bagaReadProcess(pid,&error);
    if (error) return cleanResponse(@{@"error":error});
    NSDictionary *expected=@{@"pid":@(pid),@"created":@(created),@"executable":@(executable)};
    if (!bagaSameProcess(expected,row)) return cleanResponse(@{@"ok":@YES});
    if (kill(pid,signal)!=0 && !bagaProcessGone(pid)) return cleanResponse(@{@"error":@"无法结束已核对的客户端进程。"});
    return cleanResponse(@{@"ok":@YES});
}
// ----- 只识别 Claude 的已知服务，发现阶段不读取密码 -----
static BOOL credentialService(NSString *service) {
    return [service isEqualToString:@"Claude Code-credentials"] ||
        [service hasPrefix:@"Claude Code-credentials-"] ||
        [service isEqualToString:@"Claude Safe Storage"];
}
char *rf_credentials_list(void) {
    LAContext *context = [LAContext new];
    context.interactionNotAllowed = YES;
    NSDictionary *query = @{(__bridge id)kSecClass:(__bridge id)kSecClassGenericPassword,
        (__bridge id)kSecMatchLimit:(__bridge id)kSecMatchLimitAll,
        (__bridge id)kSecReturnAttributes:@YES, (__bridge id)kSecReturnPersistentRef:@YES,
        (__bridge id)kSecUseAuthenticationContext:context};
    CFTypeRef raw = NULL;
    OSStatus status = SecItemCopyMatching((__bridge CFDictionaryRef)query, &raw);
    if (status == errSecItemNotFound) return cleanResponse(@{@"items":@[]});
    if (status != errSecSuccess) return credentialError(status, @"无法检查钥匙串属性，未读取凭据内容");
    NSArray *rows = CFBridgingRelease(raw);
    NSMutableArray *items = [NSMutableArray array];
    for (NSDictionary *row in rows) {
        NSString *service = row[(__bridge id)kSecAttrService];
        NSData *reference = row[(__bridge id)kSecValuePersistentRef];
        if (credentialService(service) && reference) {
            [items addObject:@{@"service":service,@"reference":[reference base64EncodedStringWithOptions:0]}];
        }
    }
    return cleanResponse(@{@"items":items});
}
static char *credentialError(OSStatus status, NSString *stage) {
    NSString *reason;
    if(status==errSecUserCanceled)reason=@"已取消系统授权；余下凭据未处理。";
    else if(status==errSecInteractionNotAllowed)reason=@"系统未允许授权交互，请在设置中解锁钥匙串后重试。";
    else if(status==errSecInvalidOwnerEdit)reason=@"该凭据的所有者控制拒绝删除，请在钥匙串访问中确认这一条项目；未修改所有者或访问控制。";
    else if(status==errSecAuthFailed)reason=@"钥匙串拒绝授权，请在钥匙串访问中检查该项访问控制。";
    else if(status==errSecItemNotFound)reason=@"凭据已变化，请重新扫描。";
    else reason=CFBridgingRelease(SecCopyErrorMessageString(status,NULL))?:@"系统操作失败。";
    return cleanResponse(@{@"error":[NSString stringWithFormat:@"%@：%@（OSStatus %d）",stage,reason,(int)status]});
}
char *rf_credentials_remove(const char *value) {
    NSData *reference = [[NSData alloc] initWithBase64EncodedString:[NSString stringWithUTF8String:value] options:0];
    if (!reference.length) return cleanResponse(@{@"error":@"凭据编号无效，请重新扫描。"});
    LAContext *context=[LAContext new];context.interactionNotAllowed=NO;
    NSDictionary *query = @{(__bridge id)kSecValuePersistentRef:reference,
        (__bridge id)kSecReturnAttributes:@YES,(__bridge id)kSecReturnRef:@YES,
        (__bridge id)kSecUseAuthenticationContext:context};
    CFTypeRef raw = NULL;
    OSStatus status = SecItemCopyMatching((__bridge CFDictionaryRef)query, &raw);
    if (status != errSecSuccess) return credentialError(status, @"无法核验凭据属性");
    NSDictionary *row = CFBridgingRelease(raw);
    if (!credentialService(row[(__bridge id)kSecAttrService])) return cleanResponse(@{@"error":@"凭据不属于 Claude，拒绝删除。"});
    id item = row[(__bridge id)kSecValueRef];
    if (!item) return cleanResponse(@{@"error":@"无法取得这一条凭据的系统引用，请重新扫描。"});
    // ----- 文件钥匙串使用原生单项删除；SecItemDelete 的共享密码保护会返回 -25244 -----
    // 这与 Apple security 工具相同，仍由 Security 校验实际删除权限，不修改所有者或 ACL。
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    if(CFGetTypeID((__bridge CFTypeRef)item)==SecKeychainItemGetTypeID())
        status=SecKeychainItemDelete((__bridge SecKeychainItemRef)item);
    else status=SecItemDelete((__bridge CFDictionaryRef)@{(__bridge id)kSecClass:(__bridge id)kSecClassGenericPassword,
        (__bridge id)kSecMatchItemList:@[item]});
#pragma clang diagnostic pop
    if (status != errSecSuccess) return credentialError(status, @"钥匙串删除失败");
    return cleanResponse(@{@"ok":@YES});
}
