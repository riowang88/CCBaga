/*
 * [INPUT]: Foundation/CoreLocation 状态、SystemConfiguration 代理、LaunchServices 注册/默认处理程序与 proc 进程身份。
 * [OUTPUT]: 含有效区域国家的机器快照、浏览器注册/默认处理程序与系统设置、带生命周期的资料查询（running 为 JSON 布尔值）、未知锁保留和正常退出。
 * [POS]: 浏览器的只读机器来源和进程边界；不伪造硬件，不读取浏览器凭据。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
#import <AppKit/AppKit.h>
#import <CoreServices/CoreServices.h>
#import <CoreLocation/CoreLocation.h>
#import <SystemConfiguration/SystemConfiguration.h>
#import <libproc.h>
#import <sys/sysctl.h>
#import <unistd.h>
static char *browserResponse(NSDictionary *value) {
    NSData *data = [NSJSONSerialization dataWithJSONObject:value options:0 error:NULL];
    return strdup([[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding].UTF8String);
}
char *rf_browser_register(const char *path) {
    NSURL *url=[NSURL fileURLWithPath:[NSString stringWithUTF8String:path] isDirectory:YES];
    OSStatus status=LSRegisterURL((__bridge CFURLRef)url, true);
    if(status!=noErr)return browserResponse(@{@"error":[NSString stringWithFormat:@"浏览器应用注册失败（OSStatus %d）。",(int)status]});
    return browserResponse(@{@"ok":@YES});
}
char *rf_browser_app_running(const char *bundle) {
    NSString *identifier=[NSString stringWithUTF8String:bundle];
    BOOL running=NO;
    for(NSRunningApplication *app in [NSRunningApplication runningApplicationsWithBundleIdentifier:identifier])
        if(!app.terminated){running=YES;break;}
    return browserResponse(@{@"running":@(running)});
}
static NSString *handlerFor(NSString *scheme) {
    NSURL *app=[NSWorkspace.sharedWorkspace URLForApplicationToOpenURL:[NSURL URLWithString:[scheme stringByAppendingString:@"://example.com"]]];
    NSString *identifier=app?[NSBundle bundleWithURL:app].bundleIdentifier:nil;
    return identifier?:@"";
}
char *rf_browser_default_read(const char *bundle) {
    NSString *identifier=[NSString stringWithUTF8String:bundle];
    NSString *http=handlerFor(@"http"), *https=handlerFor(@"https");
    BOOL isDefault=[http caseInsensitiveCompare:identifier]==NSOrderedSame && [https caseInsensitiveCompare:identifier]==NSOrderedSame;
    return browserResponse(@{@"is_default":@(isDefault),@"http":http,@"https":https});
}
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
/* 系统会弹出“是否更改默认网页浏览器”确认框；用户选择前返回的是请求结果，不代表已切换。 */
char *rf_browser_set_default(const char *bundle) {
    CFStringRef identifier=CFStringCreateWithCString(NULL,bundle,kCFStringEncodingUTF8);
    OSStatus http=LSSetDefaultHandlerForURLScheme(CFSTR("http"),identifier);
    OSStatus https=LSSetDefaultHandlerForURLScheme(CFSTR("https"),identifier);
    OSStatus html=LSSetDefaultRoleHandlerForContentType(CFSTR("public.html"),kLSRolesViewer,identifier);
    OSStatus xhtml=LSSetDefaultRoleHandlerForContentType(CFSTR("public.xhtml"),kLSRolesViewer,identifier);
    CFRelease(identifier);
    if(http!=noErr||https!=noErr)
        return browserResponse(@{@"error":[NSString stringWithFormat:@"系统拒绝设置默认浏览器（http %d / https %d）。",(int)http,(int)https]});
    return browserResponse(@{@"ok":@YES,@"http":@(http),@"https":@(https),@"html":@(html),@"xhtml":@(xhtml)});
}
#pragma clang diagnostic pop
char *rf_browser_machine(void) {
    NSOperatingSystemVersion v = NSProcessInfo.processInfo.operatingSystemVersion;
    NSDictionary *proxy = CFBridgingRelease(SCDynamicStoreCopyProxies(NULL)) ?: @{};
    NSString *systemProxy = @"";
    for (NSString *kind in @[@"HTTPS", @"HTTP", @"SOCKS"]) {
        if ([proxy[[kind stringByAppendingString:@"Enable"]] boolValue]) {
            NSString *host = proxy[[kind stringByAppendingString:@"Proxy"]];
            NSNumber *port = proxy[[kind stringByAppendingString:@"Port"]];
            if (host && port) { systemProxy = [NSString stringWithFormat:@"%@://%@:%@", [kind isEqual:@"SOCKS"] ? @"socks5h" : @"http", host, port]; break; }
        }
    }
    id languages = CFBridgingRelease(CFPreferencesCopyValue(CFSTR("AppleLanguages"), kCFPreferencesAnyApplication, kCFPreferencesCurrentUser, kCFPreferencesAnyHost));
    id locale = CFBridgingRelease(CFPreferencesCopyValue(CFSTR("AppleLocale"), kCFPreferencesAnyApplication, kCFPreferencesCurrentUser, kCFPreferencesAnyHost));
    NSString *localeIdentifier = [locale isKindOfClass:NSString.class] ? locale : NSLocale.currentLocale.localeIdentifier;
    NSString *localeCountry = [[NSLocale alloc] initWithLocaleIdentifier:localeIdentifier].countryCode ?: @"";
    [NSTimeZone resetSystemTimeZone];
    return browserResponse(@{@"platform": @"macos", @"version": [NSString stringWithFormat:@"%ld.%ld.%ld", (long)v.majorVersion, (long)v.minorVersion, (long)v.patchVersion],
        @"cores": @(NSProcessInfo.processInfo.activeProcessorCount), @"timezone": NSTimeZone.systemTimeZone.name,
        @"languages": [languages isKindOfClass:NSArray.class] ? languages : NSLocale.preferredLanguages,
        @"locale": localeIdentifier, @"locale_country": localeCountry,
        @"location_enabled": @([CLLocationManager locationServicesEnabled]), @"timezones": NSTimeZone.knownTimeZoneNames,
        @"system_proxy": systemProxy});
}
#include "Processes.h"
#include <sys/stat.h>
#include <limits.h>
static NSString *browserArgumentProfile(NSArray<NSString *> *args, NSString **error) {
    NSString *profile=nil;
    for (NSUInteger i=1;i<args.count;i++) {
        NSString *arg=args[i], *value=nil;
        if ([arg isEqual:@"--"]) break;
        if ([arg hasPrefix:@"--user-data-dir="]) value=[arg substringFromIndex:16];
        else if ([arg isEqual:@"--user-data-dir"]) {
            if (++i>=args.count) { *error=@"浏览器资料参数不完整。"; return nil; }
            value=args[i];
        }
        if (value) {
            if (!value.isAbsolutePath) { *error=@"浏览器资料路径无法确认。"; return nil; }
            value=value.stringByStandardizingPath;
            if (profile && ![profile isEqual:value]) { *error=@"浏览器资料参数有冲突。"; return nil; }
            profile=value;
        }
    }
    return profile;
}
static NSNumber *browserLockPID(NSString *profile, NSString **error) {
    NSString *path=[profile stringByAppendingPathComponent:@"SingletonLock"];
    struct stat info;
    if (lstat(path.fileSystemRepresentation,&info)!=0) {
        if (errno!=ENOENT) *error=@"无法检查浏览器资料锁；未确认退出。";
        return nil;
    }
    if (!S_ISLNK(info.st_mode)) { *error=@"浏览器资料锁类型未知，已保留。"; return nil; }
    char raw[PATH_MAX+1]={0}; ssize_t length=readlink(path.fileSystemRepresentation,raw,PATH_MAX);
    if (length<=0 || length>=PATH_MAX) { *error=@"无法读取浏览器资料锁，已保留。"; return nil; }
    NSString *lock=[[NSString alloc] initWithBytes:raw length:length encoding:NSUTF8StringEncoding];
    NSRange dash=[lock rangeOfString:@"-" options:NSBackwardsSearch];
    if (!lock || dash.location==NSNotFound) { *error=@"浏览器资料锁格式未知，已保留。"; return nil; }
    NSString *number=[lock substringFromIndex:dash.location+1];
    char host[256]={0};
    if (gethostname(host,sizeof(host))!=0 || ![[lock substringToIndex:dash.location] isEqual:@(host)]) { *error=@"浏览器资料锁不属于当前主机或无法核对，已保留。"; return nil; }
    unsigned long long pid=0;
    for (NSUInteger i=0;i<number.length;i++) { unichar c=[number characterAtIndex:i]; if (c<'0'||c>'9'||pid>INT_MAX/10ULL) { *error=@"浏览器资料锁编号无效，已保留。"; return nil; } pid=pid*10+c-'0'; }
    if (pid<=1 || pid>INT_MAX) { *error=@"浏览器资料锁编号无效，已保留。"; return nil; }
    return @(pid);
}
static NSArray<NSDictionary *> *profileProcesses(NSString *profile, NSString **error) {
    profile=profile.stringByStandardizingPath;
    NSNumber *lockPID=browserLockPID(profile,error);
    if (*error) return nil;
    NSArray *rows=bagaProcessList(error); if (*error) return nil;
    NSMutableDictionary *candidates=[NSMutableDictionary dictionary];
    for (NSDictionary *row in rows) {
        NSString *root=bagaProcessApp(row[@"executable"]);
        if (!root || ![@[@"ai.local.requirement-french.browser",@"local.ccbroswer"] containsObject:BAGA_PROCESS_BUNDLE(root)]) continue;
        NSArray *args=bagaProcessArgs(row,error); if (*error) return nil;
        if (!args) continue; // 沿用内核确认退出的排除规则，不忽略未知状态。
        NSString *own=browserArgumentProfile(args,error); if (*error) return nil;
        NSMutableDictionary *candidate=[row mutableCopy];
        candidate[@"profile"]=own?:NSNull.null;
        candidate[@"app"]=root;
        candidate[@"main"]=@([@[@"Chromium",@"CCBrowser"] containsObject:[row[@"executable"] lastPathComponent]]);
        candidates[row[@"pid"]]=candidate;
    }
    NSMutableArray *matches=[NSMutableArray array];
    for (NSDictionary *row in candidates.allValues) {
        NSDictionary *parent=row; NSString *resolved=nil;
        NSMutableSet *seen=[NSMutableSet set];
        while (parent && seen.count<64 && ![seen containsObject:parent[@"pid"]]) {
            [seen addObject:parent[@"pid"]];
            if ([parent[@"profile"] isKindOfClass:NSString.class]) { resolved=parent[@"profile"]; break; }
            parent=candidates[parent[@"parent"]];
            if (parent && ![parent[@"app"] isEqual:row[@"app"]]) break;
        }
        if (!resolved) { *error=@"浏览器主进程或辅助进程仍在，但无法确定其资料目录；请完整退出浏览器后重试。"; return nil; }
        if ([resolved isEqual:profile]) [matches addObject:row];
    }
    if (lockPID) {
        BOOL owned=NO; for (NSDictionary *row in matches) if ([row[@"pid"] isEqual:lockPID]) owned=YES;
        if (!owned && !bagaProcessGone(lockPID.intValue)) { *error=@"浏览器资料锁与当前进程身份不符，已保留。"; return nil; }
    }
    return matches;
}
char *rf_browser_running(const char *profile) {
    NSString *error=nil; NSArray *rows=profileProcesses(@(profile),&error);
    return browserResponse(error?@{@"error":error}:@{@"running":rows.count>0?@YES:@NO});
}
char *rf_browser_quit(const char *profile) {
    NSString *error=nil; NSArray *rows=profileProcesses(@(profile),&error);
    if (error) return browserResponse(@{@"error":error});
    for (NSDictionary *row in rows) {
        if (![row[@"main"] boolValue]) continue;
        NSRunningApplication *app=[NSRunningApplication runningApplicationWithProcessIdentifier:[row[@"pid"] intValue]];
        NSDictionary *current=bagaReadProcess([row[@"pid"] intValue],&error);
        if (error) return browserResponse(@{@"error":error});
        if (!bagaSameProcess(row,current)) return browserResponse(@{@"error":@"浏览器进程在退出核对期间已变化，请重试。"});
        if (!app || ![app terminate]) return browserResponse(@{@"error":@"浏览器未接受退出，请在浏览器窗口保存任务后退出。"});
    }
    return browserResponse(@{@"ok":@YES});
}
