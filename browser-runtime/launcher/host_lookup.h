/*
 * [INPUT]: B 应用包所在目录（如 /Applications）与宿主 Bundle ID。
 * [OUTPUT]: find_host_app：按 RioBaga-A.app → 旧名 CCBaga-A.app 顺序返回第一个是目录且 Info.plist 的
 *           CFBundleIdentifier 等于宿主标识的应用包路径；都不合格返回 0，由调用方按 Bundle ID 交给 LaunchServices。
 * [POS]: CCBrowser.c 的纯查找部分，单独成头文件以便 scripts/test-browser-launcher.py 用临时目录编译测试。
 *        旧名兼容自动更新后仍保留 CCBaga-A.app 的老用户；名字对但标识不符不采用。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
#ifndef CCBAGA_HOST_LOOKUP_H
#define CCBAGA_HOST_LOOKUP_H
#include <CoreFoundation/CoreFoundation.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>

static const char *const HOST_APP_NAMES[] = {"RioBaga-A.app", "CCBaga-A.app"};

static int bundle_has_id(const char *app, const char *expected) {
    CFURLRef url = CFURLCreateFromFileSystemRepresentation(NULL, (const UInt8 *)app, (CFIndex)strlen(app), true);
    if (!url) return 0;
    CFBundleRef bundle = CFBundleCreate(NULL, url);
    CFRelease(url);
    if (!bundle) return 0;
    int ok = 0;
    CFStringRef id = CFBundleGetIdentifier(bundle);
    char buffer[256];
    if (id && CFStringGetCString(id, buffer, sizeof(buffer), kCFStringEncodingUTF8)) ok = strcmp(buffer, expected) == 0;
    CFRelease(bundle);
    return ok;
}

static int find_host_app(const char *apps_dir, const char *host_id, char *out, size_t size) {
    for (size_t i = 0; i < sizeof(HOST_APP_NAMES) / sizeof(HOST_APP_NAMES[0]); i++) {
        struct stat st;
        int written = snprintf(out, size, "%s/%s", apps_dir, HOST_APP_NAMES[i]);
        if (written < 0 || (size_t)written >= size) continue;
        if (stat(out, &st) == 0 && S_ISDIR(st.st_mode) && bundle_has_id(out, host_id)) return 1;
    }
    if (size) out[0] = 0;
    return 0;
}
#endif
