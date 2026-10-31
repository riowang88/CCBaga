/*
 * [INPUT]: 临时钥匙串、虚构默认/自定义/无关服务和原生 Identity 桥。
 * [OUTPUT]: 范围/精确删除恢复/陈旧拒绝；旧式 ACL 更新保留、现代签名分区在相同密钥下不重写的回归。
 * [POS]: 替换默认钥匙串获取的离线验证；不改系统默认钥匙串，不使用真实账号。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
#import <Foundation/Foundation.h>
#import <Security/Security.h>
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
static SecKeychainRef fixture=NULL;
static OSStatus fixtureDefault(SecKeychainRef *out){*out=fixture;CFRetain(fixture);return errSecSuccess;}
#define BAGA_IDENTITY_KEYCHAIN fixtureDefault
#include "Identity.m"
static NSDictionary *json(char *value){NSData *bytes=[[NSString stringWithUTF8String:value] dataUsingEncoding:NSUTF8StringEncoding];free(value);return [NSJSONSerialization JSONObjectWithData:bytes options:0 error:nil];}
static NSString *payload;
static const char *input(id value){payload=[[NSString alloc]initWithData:[NSJSONSerialization dataWithJSONObject:value options:0 error:nil] encoding:NSUTF8StringEncoding];return payload.UTF8String;}
static void add(NSString *service,NSString *account,NSString *value){
    NSCAssert(SecItemAdd((__bridge CFDictionaryRef)@{(__bridge id)kSecClass:(__bridge id)kSecClassGenericPassword,(__bridge id)kSecAttrService:service,(__bridge id)kSecAttrAccount:account,(__bridge id)kSecUseKeychain:(__bridge id)fixture,(__bridge id)kSecValueData:[value dataUsingEncoding:NSUTF8StringEncoding]},NULL)==errSecSuccess,@"fixture create failed");
}
static NSDictionary *desktopItem(NSString *account){
    for(NSDictionary *row in json(rf_identity_snapshot_for(2))[@"items"])
        if([row[@"account"] isEqual:account])return row;
    return nil;
}
static NSArray *accessSnapshot(NSDictionary *saved){
    NSData *ref=[[NSData alloc]initWithBase64EncodedString:saved[@"reference"] options:0];
    CFTypeRef item=NULL;SecAccessRef access=NULL;CFArrayRef acls=NULL;
    NSCAssert(SecItemCopyMatching((__bridge CFDictionaryRef)@{(__bridge id)kSecValuePersistentRef:ref,(__bridge id)kSecReturnRef:@YES},&item)==errSecSuccess,@"fixture item lookup");
    NSCAssert(SecKeychainItemCopyAccess((SecKeychainItemRef)item,&access)==errSecSuccess,@"fixture ACL lookup");
    NSCAssert(SecAccessCopyACLList(access,&acls)==errSecSuccess,@"fixture ACL list");
    NSMutableArray *result=[NSMutableArray array];
    for(id value in (__bridge NSArray *)acls){
        SecACLRef acl=(__bridge SecACLRef)value;CFArrayRef apps=NULL;CFStringRef description=NULL;SecKeychainPromptSelector prompt=0;
        NSCAssert(SecACLCopyContents(acl,&apps,&description,&prompt)==errSecSuccess,@"fixture ACL contents");
        NSMutableArray *trusted=[NSMutableArray array];
        for(id app in (__bridge NSArray *)apps){CFDataRef data=NULL;
            NSCAssert(SecTrustedApplicationCopyData((__bridge SecTrustedApplicationRef)app,&data)==errSecSuccess,@"fixture trusted identity");
            [trusted addObject:[CFBridgingRelease(data) base64EncodedStringWithOptions:0]];
        }
        [result addObject:@{@"apps":apps?trusted:NSNull.null,@"description":(__bridge id)description?:@"",@"prompt":@(prompt),@"operations":CFBridgingRelease(SecACLCopyAuthorizations(acl))?:@[]}];
        if(apps)CFRelease(apps);if(description)CFRelease(description);
    }
    CFRelease(acls);CFRelease(access);CFRelease(item);return result;
}
static void checkDesktopAccess(NSString *path){
    // A different signed executable creates the item and explicitly trusts this test.
    // All values and the keychain are disposable; no login keychain is searched.
    NSTask *creator=[[NSTask alloc]init];creator.executableURL=[NSURL fileURLWithPath:@"/usr/bin/security"];
    creator.arguments=@[@"add-generic-password",@"-s",@"Claude Safe Storage",@"-a",@"acl-fixture",@"-l",@"fixture persisted authorization",@"-w",@"fixture-A",@"-T",@"/usr/bin/security",@"-T",NSProcessInfo.processInfo.arguments[0],path];
    NSError *error=nil;NSCAssert([creator launchAndReturnError:&error],@"fixture creator launch");[creator waitUntilExit];
    NSCAssert(creator.terminationStatus==0,@"fixture creator failed");
    NSDictionary *a=desktopItem(@"acl-fixture");NSCAssert(a,@"foreign fixture missing");
    NSArray *access=accessSnapshot(a);
    NSMutableDictionary *prepared=[a mutableCopy];prepared[@"remaining"]=a[@"data"];
    NSCAssert([json(rf_identity_remove(input(@[prepared])))[@"ok"] boolValue],@"prepare same desktop entry failed");
    NSCAssert([desktopItem(@"acl-fixture")[@"reference"] isEqual:a[@"reference"]],@"prepare recreated the item");
    NSMutableDictionary *b=[a mutableCopy];b[@"data"]=[[@"fixture-B" dataUsingEncoding:NSUTF8StringEncoding] base64EncodedStringWithOptions:0];b[@"expected"]=a[@"data"];
    NSCAssert([json(rf_identity_restore(input(@[b])))[@"ok"] boolValue],@"desktop update failed");
    NSDictionary *live=desktopItem(@"acl-fixture");
    NSCAssert([live[@"data"] isEqual:b[@"data"]] && [live[@"reference"] isEqual:a[@"reference"]],@"desktop data must change in the same item");
    NSCAssert([accessSnapshot(live) isEqual:access],@"desktop update changed application grants, prompts or operations");
    NSCAssert(json(rf_identity_remove(input(@[prepared])))[@"error"],@"stale preparation must refuse the changed secret");
    NSMutableDictionary *stale=[a mutableCopy];stale[@"expected"]=a[@"data"];
    NSCAssert(json(rf_identity_restore(input(@[stale])))[@"error"],@"stale restore overwrote a different value");
    NSMutableDictionary *rollback=[a mutableCopy];rollback[@"expected"]=b[@"data"];
    NSCAssert([json(rf_identity_restore(input(@[rollback])))[@"ok"] boolValue],@"desktop rollback failed");
    NSCAssert([accessSnapshot(desktopItem(@"acl-fixture")) isEqual:access],@"rollback changed authorization");
    NSCAssert([json(rf_identity_remove(input(@[desktopItem(@"acl-fixture")])))[@"ok"] boolValue],@"empty identity must still remove the credential");
    NSCAssert(!desktopItem(@"acl-fixture"),@"empty identity retained the credential");
    NSCAssert([json(rf_identity_restore(input(@[a])))[@"ok"] boolValue],@"restore absent desktop entry failed");
    NSCAssert(![accessSnapshot(desktopItem(@"acl-fixture")) isEqual:access],@"baseline delete/add must demonstrate loss of the old grants");
    NSCAssert([json(rf_identity_remove(input(@[desktopItem(@"acl-fixture")])))[@"ok"] boolValue],@"fixture cleanup");
}
static void checkModernNoop(void){
    // securityd enables partition ACLs under Library/Keychains; a plain /tmp
    // keychain exercises the older format and cannot prove this behavior.
    NSString *root=[NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSString *folder=[root stringByAppendingPathComponent:@"Library/Keychains"];
    NSCAssert([NSFileManager.defaultManager createDirectoryAtPath:folder withIntermediateDirectories:YES attributes:@{NSFilePosixPermissions:@0700} error:nil],@"modern fixture directory");
    NSString *path=[folder stringByAppendingPathComponent:@"fixture.keychain-db"];
    NSCAssert(SecKeychainCreate(path.UTF8String,15,"disposable-only",FALSE,NULL,&fixture)==errSecSuccess,@"modern fixture create");
    @try{
        add(@"Claude Safe Storage",@"modern-fixture",@"fixture-storage");
        SecCodeRef code=NULL;CFDictionaryRef info=NULL;
        NSCAssert(SecCodeCopySelf(0,&code)==errSecSuccess,@"test code identity");
        NSCAssert(SecCodeCopySigningInformation(code,kSecCSSigningInformation,&info)==errSecSuccess,@"test signing information");
        NSData *hash=((__bridge NSDictionary *)info)[(__bridge id)kSecCodeInfoUnique];
        NSCAssert(hash.length>0,@"test executable must be signed");
        NSMutableString *hex=[NSMutableString string];for(NSUInteger i=0;i<hash.length;i++)[hex appendFormat:@"%02x",((const unsigned char *)hash.bytes)[i]];
        CFRelease(info);CFRelease(code);
        NSTask *setter=[[NSTask alloc]init];setter.executableURL=[NSURL fileURLWithPath:@"/usr/bin/security"];
        setter.arguments=@[@"set-generic-password-partition-list",@"-s",@"Claude Safe Storage",@"-a",@"modern-fixture",@"-S",[@"apple:,cdhash:" stringByAppendingString:hex],@"-k",@"disposable-only",path];
        setter.standardOutput=[NSFileHandle fileHandleWithNullDevice];
        NSCAssert([setter launchAndReturnError:nil],@"fixture partition setup");[setter waitUntilExit];NSCAssert(setter.terminationStatus==0,@"fixture partition setup failed");
        NSDictionary *saved=desktopItem(@"modern-fixture");NSArray *before=accessSnapshot(saved);
        NSUInteger partitions=0;for(NSDictionary *acl in before)if([acl[@"operations"] containsObject:@"ACLAuthorizationPartitionID"])partitions++;
        NSCAssert(partitions==1,@"modern fixture lacks signature partition ACL");
        NSMutableDictionary *prepare=[saved mutableCopy];prepare[@"remaining"]=saved[@"data"];
        for(int i=0;i<3;i++){
            NSCAssert([json(rf_identity_remove(input(@[prepare])))[@"ok"] boolValue],@"same-secret preparation failed");
            NSCAssert([json(rf_identity_restore(input(@[saved])))[@"ok"] boolValue],@"same-secret restore failed");
            NSCAssert([desktopItem(@"modern-fixture")[@"reference"] isEqual:saved[@"reference"]],@"same-secret item recreated");
            NSCAssert([accessSnapshot(saved) isEqual:before],@"same-secret transition reset full partition ACL");
        }
    }@finally{SecKeychainDelete(fixture);CFRelease(fixture);fixture=NULL;[NSFileManager.defaultManager removeItemAtPath:root error:nil];}
}
int main(void){@autoreleasepool{
    NSString *path=[NSTemporaryDirectory() stringByAppendingPathComponent:[NSUUID.UUID.UUIDString stringByAppendingString:@".keychain-db"]];
    const char *password="disposable-only";
    NSCAssert(SecKeychainCreate(path.UTF8String,(UInt32)strlen(password),password,FALSE,NULL,&fixture)==errSecSuccess,@"isolated keychain create failed");
    Boolean previousInteraction=TRUE;
    NSCAssert(SecKeychainGetUserInteractionAllowed(&previousInteraction)==errSecSuccess,@"fixture interaction state");
    @try{
        // Prevent an unexpected test-only access check from opening a password dialog.
        NSCAssert(SecKeychainSetUserInteractionAllowed(FALSE)==errSecSuccess,@"fixture noninteractive mode");
        add(@"Claude Code-credentials",@"first",@"{\"claudeAiOauth\":{\"accessToken\":\"fixture\"},\"mcpOAuth\":\"keep\"}");
        add(@"Claude Code-credentials",@"sibling",@"{\"claudeAiOauth\":{\"accessToken\":\"sibling\"}}");
        add(@"Claude Code",@"first",@"fixture-api");add(@"Claude Safe Storage",@"first",@"fixture-storage");
        add(@"Claude Code-credentials-custom",@"first",@"custom-kept");add(@"unrelated-fixture",@"first",@"unrelated-kept");
        NSArray *saved=json(rf_identity_snapshot())[@"items"];NSCAssert(saved.count==4,@"snapshot escaped default services");
        NSArray *cli=json(rf_identity_snapshot_for(1))[@"items"],*desktop=json(rf_identity_snapshot_for(2))[@"items"];
        NSCAssert(cli.count==3 && desktop.count==1,@"single-client scope incorrect");
        for(NSDictionary *row in cli)NSCAssert(![row[@"service"] isEqual:@"Claude Safe Storage"],@"CLI snapshot absorbed Desktop");
        NSCAssert([desktop[0][@"service"] isEqual:@"Claude Safe Storage"],@"Desktop snapshot absorbed CLI");
        NSCAssert(json(rf_identity_snapshot_for(3))[@"error"]!=nil,@"invalid scope accepted");
        NSDictionary *first=nil;for(NSDictionary *row in saved)if([row[@"service"] isEqual:@"Claude Code-credentials"]&&[row[@"account"] isEqual:@"first"])first=row;
        NSCAssert(first!=nil,@"Code fixture missing");
        NSMutableDictionary *partial=[first mutableCopy];partial[@"remaining"]=[[@"{\"mcpOAuth\":\"keep\"}" dataUsingEncoding:NSUTF8StringEncoding] base64EncodedStringWithOptions:0];
        NSCAssert([json(rf_identity_remove(input(@[partial])))[@"ok"] boolValue],@"partial clear failed");
        NSCAssert(json(rf_identity_remove(input(@[first])))[@"error"]!=nil,@"changed credential was removed");
        NSMutableDictionary *restore=[first mutableCopy];restore[@"expected"]=partial[@"remaining"];
        NSCAssert([json(rf_identity_restore(input(@[restore])))[@"ok"] boolValue],@"conditional restore failed");
        NSCAssert([json(rf_identity_remove(input(@[first])))[@"ok"] boolValue],@"exact clear failed");
        NSCAssert([json(rf_identity_snapshot())[@"items"] count]==3,@"sibling was removed");
        NSCAssert([json(rf_identity_restore(input(@[first])))[@"ok"] boolValue],@"restore absent credential failed");
        NSCAssert([json(rf_identity_snapshot())[@"items"] count]==4,@"restore missing");
        NSCAssert([json(rf_identity_remove(input(saved)))[@"ok"] boolValue],@"default entries clear failed");
        NSCAssert([json(rf_identity_snapshot())[@"items"] count]==0,@"default entries remain");
        for(NSString *service in @[@"Claude Code-credentials-custom",@"unrelated-fixture"]){
            CFTypeRef raw=NULL;NSCAssert(SecItemCopyMatching((__bridge CFDictionaryRef)@{(__bridge id)kSecClass:(__bridge id)kSecClassGenericPassword,(__bridge id)kSecAttrService:service,(__bridge id)kSecMatchSearchList:@[(__bridge id)fixture],(__bridge id)kSecReturnAttributes:@YES},&raw)==errSecSuccess,@"unrelated/custom entry changed");CFRelease(raw);
        }
        checkDesktopAccess(path);
    }@finally{SecKeychainDelete(fixture);CFRelease(fixture);fixture=NULL;}
    @try{checkModernNoop();}@finally{SecKeychainSetUserInteractionAllowed(previousInteraction);}
    puts("Isolated Keychain checks passed: scope, stale refusal, exact deletion, legacy ACL update/rollback and full modern partition ACL preservation for repeated unchanged-secret transitions. Changed-secret modern partition preservation is NOT covered.");
}return 0;}
