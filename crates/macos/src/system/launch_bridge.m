#import <AppKit/AppKit.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    void *application;
    int32_t pid;
    double launchTime;
    uint8_t terminated;
    uint8_t deliveryStarted;
    uint8_t errorKind;
    const char *error;
    size_t errorLength;
} AgentDesktopLaunchResult;

typedef void (*AgentDesktopLaunchCompletion)(void *, const AgentDesktopLaunchResult *);
typedef void (*AgentDesktopLaunchRelease)(void *);

typedef struct {
    void *context;
    AgentDesktopLaunchRelease releaseContext;
    bool released;
} AgentDesktopLaunchReleaseBox;

static void releaseBox(AgentDesktopLaunchReleaseBox *box) {
    if (box == NULL) {
        return;
    }
    if (!box->released) {
        box->released = true;
        box->releaseContext(box->context);
    }
    free(box);
}

@interface AgentDesktopLaunchContext : NSObject {
@public
    AgentDesktopLaunchReleaseBox *_releaseBox;
    AgentDesktopLaunchCompletion _completion;
}
@end

@implementation AgentDesktopLaunchContext
- (void)dealloc {
    @try {
        releaseBox(_releaseBox);
        _releaseBox = NULL;
    } @catch (NSException *exception) {
        (void)exception;
    }
}
@end

static void completeError(
    AgentDesktopLaunchContext *holder,
    NSString *message,
    uint8_t kind,
    uint8_t deliveryStarted
) {
    const char *bytes = message.UTF8String;
    AgentDesktopLaunchResult result = {
        .application = NULL,
        .pid = 0,
        .launchTime = 0.0,
        .terminated = true,
        .deliveryStarted = deliveryStarted,
        .errorKind = kind,
        .error = bytes,
        .errorLength = bytes == NULL ? 0 : strlen(bytes),
    };
    holder->_completion(holder->_releaseBox->context, &result);
}

static BOOL isStringArray(id value) {
    if (![value isKindOfClass:[NSArray class]]) {
        return NO;
    }
    for (id item in (NSArray *)value) {
        if (![item isKindOfClass:[NSString class]]) {
            return NO;
        }
    }
    return YES;
}

static NSString *withoutAppExtension(NSString *name) {
    if ([name.pathExtension compare:@"app" options:NSCaseInsensitiveSearch] == NSOrderedSame) {
        return name.stringByDeletingPathExtension;
    }
    return name;
}

static BOOL sameApplicationName(NSString *shown, NSString *requested) {
    NSStringCompareOptions options = NSCaseInsensitiveSearch | NSDiacriticInsensitiveSearch;
    return [withoutAppExtension(shown) compare:withoutAppExtension(requested)
                                       options:options] == NSOrderedSame;
}

static NSArray<NSString *> *applicationDirectories(void) {
    NSString *home = [NSHomeDirectory() stringByAppendingPathComponent:@"Applications"];
    return @[
        home,
        @"/Applications",
        @"/Applications/Utilities",
        @"/System/Applications",
        @"/System/Applications/Utilities",
    ];
}

static NSDictionary *infoStringsForLocalization(NSBundle *bundle, NSDictionary *table, NSString *localization) {
    id strings = table[localization];
    if (![strings isKindOfClass:[NSDictionary class]]) {
        NSString *file = [bundle pathForResource:@"InfoPlist"
                                          ofType:@"strings"
                                     inDirectory:nil
                                 forLocalization:localization];
        strings = file == nil ? nil : [NSDictionary dictionaryWithContentsOfFile:file];
    }
    return [strings isKindOfClass:[NSDictionary class]] ? strings : nil;
}

/// The names the user sees for an application in their own language. A command-line
/// process has no localizations of its own, so `displayNameAtPath:` answers in English
/// (`Calculator`); the translated `CFBundleDisplayName` and `CFBundleName` come from the
/// bundle's `InfoPlist.loctable` or `InfoPlist.strings` for the user's languages.
static NSArray<NSString *> *localizedApplicationNames(NSString *path, NSArray<NSString *> *languages) {
    NSMutableArray<NSString *> *names = [NSMutableArray array];
    NSBundle *bundle = [NSBundle bundleWithPath:path];
    if (bundle != nil) {
        NSString *tablePath = [bundle pathForResource:@"InfoPlist" ofType:@"loctable"];
        NSDictionary *table = tablePath == nil ? nil : [NSDictionary dictionaryWithContentsOfFile:tablePath];
        NSArray<NSString *> *localizations =
            [NSBundle preferredLocalizationsFromArray:bundle.localizations
                                       forPreferences:languages];
        for (NSString *localization in localizations) {
            NSDictionary *strings = infoStringsForLocalization(bundle, table, localization);
            for (NSString *key in @[@"CFBundleDisplayName", @"CFBundleName"]) {
                if ([strings[key] isKindOfClass:[NSString class]]) {
                    [names addObject:strings[key]];
                }
            }
        }
    }
    NSString *shown = [[NSFileManager defaultManager] displayNameAtPath:path];
    if (shown.length > 0) {
        [names addObject:shown];
    }
    return names;
}

/// Finds an application by a name the user sees for it, which on a Mac set to another
/// language is the translated name (`Calculadora` for `Calculator.app`).
/// `fullPathForApplication:` only knows the bundle file name, so this lookup is what lets
/// `launch` take the name that `list-apps` reports and `--app` accepts. Case and accents
/// are ignored and the first match in `directories` order wins.
static NSString *applicationPathForDisplayName(
    NSString *requested,
    NSArray<NSString *> *directories,
    NSArray<NSString *> *(^namesForPath)(NSString *path)
) {
    NSFileManager *files = [NSFileManager defaultManager];
    for (NSString *directory in directories) {
        NSArray<NSString *> *entries = [[files contentsOfDirectoryAtPath:directory error:nil]
            sortedArrayUsingSelector:@selector(compare:)];
        for (NSString *entry in entries) {
            if ([entry.pathExtension compare:@"app" options:NSCaseInsensitiveSearch] != NSOrderedSame) {
                continue;
            }
            NSString *path = [directory stringByAppendingPathComponent:entry];
            for (NSString *shown in namesForPath(path)) {
                if (shown.length > 0 && sameApplicationName(shown, requested)) {
                    return path;
                }
            }
        }
    }
    return nil;
}

static BOOL isStringDictionary(id value) {
    if (![value isKindOfClass:[NSDictionary class]]) {
        return NO;
    }
    for (id key in (NSDictionary *)value) {
        if (![key isKindOfClass:[NSString class]] ||
            ![value[key] isKindOfClass:[NSString class]]) {
            return NO;
        }
    }
    return YES;
}

bool agent_desktop_open_application(
    const uint8_t *requestBytes,
    size_t requestLength,
    void *context,
    AgentDesktopLaunchCompletion completion,
    AgentDesktopLaunchRelease releaseContext
) {
    AgentDesktopLaunchReleaseBox *box = calloc(1, sizeof(*box));
    if (box == NULL) {
        releaseContext(context);
        return false;
    }
    box->context = context;
    box->releaseContext = releaseContext;
    AgentDesktopLaunchContext *holder = nil;
    @try {
        holder = [AgentDesktopLaunchContext new];
        if (holder == nil) {
            releaseBox(box);
            return false;
        }
        holder->_releaseBox = box;
        holder->_completion = completion;
        box = NULL;
        NSData *data = [NSData dataWithBytes:requestBytes length:requestLength];
        NSError *jsonError = nil;
        id decoded = [NSJSONSerialization JSONObjectWithData:data options:0 error:&jsonError];
        if (![decoded isKindOfClass:[NSDictionary class]]) {
            completeError(holder, jsonError.localizedDescription ?: @"Invalid launch request", 2, 0);
            return true;
        }
        NSDictionary *request = (NSDictionary *)decoded;
        NSString *identifier = request[@"identifier"];
        NSArray *arguments = request[@"arguments"];
        NSDictionary *environment = request[@"environment"];
        if (![identifier isKindOfClass:[NSString class]] ||
            !isStringArray(arguments) || !isStringDictionary(environment)) {
            completeError(holder, @"Invalid launch request fields", 2, 0);
            return true;
        }
        NSWorkspace *workspace = [NSWorkspace sharedWorkspace];
        NSURL *url = nil;
        if ([request[@"bundle_id"] boolValue]) {
            url = [workspace URLForApplicationWithBundleIdentifier:identifier];
        } else {
            NSString *path = [workspace fullPathForApplication:identifier];
            if (path == nil) {
                path = applicationPathForDisplayName(
                    identifier, applicationDirectories(), ^(NSString *candidate) {
                        return localizedApplicationNames(candidate, [NSLocale preferredLanguages]);
                    });
            }
            if (path != nil) {
                url = [NSURL fileURLWithPath:path];
            }
        }
        if (url == nil) {
            completeError(holder, @"Application bundle was not found", 1, 0);
            return true;
        }
        NSWorkspaceOpenConfiguration *configuration =
            [NSWorkspaceOpenConfiguration configuration];
        configuration.activates = [request[@"activates"] boolValue];
        configuration.promptsUserIfNeeded = [request[@"prompts"] boolValue];
        configuration.allowsRunningApplicationSubstitution =
            [request[@"substitution"] boolValue];
        configuration.createsNewApplicationInstance = [request[@"new_instance"] boolValue];
        configuration.arguments = arguments;
        configuration.environment = environment;
        [workspace openApplicationAtURL:url
                         configuration:configuration
                     completionHandler:^(NSRunningApplication *app, NSError *error) {
            @try {
                if (error != nil || app == nil) {
                    completeError(
                        holder,
                        error.localizedDescription ?: @"Launch Services returned no application",
                        4,
                        1
                    );
                    return;
                }
                NSDate *launchDate = app.launchDate;
                AgentDesktopLaunchResult result = {
                    .application = (__bridge void *)app,
                    .pid = app.processIdentifier,
                    .launchTime = launchDate == nil ? 0.0 : launchDate.timeIntervalSince1970,
                    .terminated = app.isTerminated,
                    .deliveryStarted = 1,
                    .errorKind = 0,
                    .error = NULL,
                    .errorLength = 0,
                };
                holder->_completion(holder->_releaseBox->context, &result);
            } @catch (NSException *exception) {
                completeError(holder, exception.reason ?: @"Launch callback exception", 3, 1);
            }
        }];
    } @catch (NSException *exception) {
        if (holder != nil && holder->_releaseBox != NULL) {
            @try {
                completeError(holder, exception.reason ?: @"Launch bridge exception", 3, 0);
            } @catch (NSException *completionException) {
                (void)completionException;
            }
            return true;
        }
        releaseBox(box);
        return false;
    }
    return true;
}

uint8_t agent_desktop_running_application_is_live(void *application, int32_t expectedPID) {
    @try {
        NSRunningApplication *app = (__bridge NSRunningApplication *)application;
        return app != nil && !app.isTerminated && app.processIdentifier == expectedPID;
    } @catch (NSException *exception) {
        (void)exception;
        return 0;
    }
}
