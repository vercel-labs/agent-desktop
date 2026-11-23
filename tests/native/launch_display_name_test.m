#import "../../crates/macos/src/system/launch_bridge.m"

static void require(bool condition, const char *message) {
    if (!condition) {
        fprintf(stderr, "%s\n", message);
        exit(1);
    }
}

static NSString *makeDirectory(NSString *root, NSString *name) {
    NSString *path = [root stringByAppendingPathComponent:name];
    BOOL created = [[NSFileManager defaultManager] createDirectoryAtPath:path
                                             withIntermediateDirectories:YES
                                                              attributes:nil
                                                                   error:nil];
    require(created, "create test directory");
    return path;
}

static void writePlist(NSDictionary *value, NSString *path) {
    NSData *data = [NSPropertyListSerialization dataWithPropertyList:value
                                                              format:NSPropertyListBinaryFormat_v1_0
                                                             options:0
                                                               error:nil];
    require(data != nil && [data writeToFile:path atomically:YES], "write property list");
}

static NSString *makeBundle(NSString *directory, NSString *fileName, NSString *englishName) {
    NSString *bundle = makeDirectory(directory, fileName);
    NSString *contents = makeDirectory(bundle, @"Contents");
    makeDirectory(contents, @"Resources/en.lproj");
    makeDirectory(contents, @"Resources/es.lproj");
    writePlist(@{
        @"CFBundleIdentifier": [@"dev.example." stringByAppendingString:englishName.lowercaseString],
        @"CFBundleName": englishName,
        @"CFBundlePackageType": @"APPL",
    }, [contents stringByAppendingPathComponent:@"Info.plist"]);
    return bundle;
}

static NSString *temporaryRoot(void) {
    NSString *template = [NSTemporaryDirectory() stringByAppendingPathComponent:@"launch-name-XXXXXX"];
    char *buffer = strdup(template.fileSystemRepresentation);
    require(buffer != NULL && mkdtemp(buffer) != NULL, "temporary directory");
    NSString *root = [NSString stringWithUTF8String:buffer];
    free(buffer);
    return root;
}

static void matchesNamesFromAnyDirectory(NSString *root) {
    NSString *first = makeDirectory(root, @"First");
    NSString *second = makeDirectory(root, @"Second");
    NSString *calculator = makeDirectory(first, @"Calculator.app");
    NSString *settings = makeDirectory(first, @"System Settings.app");
    NSString *notes = makeDirectory(first, @"Notes.bundle");
    NSString *twin = makeDirectory(second, @"Calculadora.app");
    NSDictionary<NSString *, NSArray<NSString *> *> *shown = @{
        calculator: @[@"Calculator", @"Calculadora"],
        settings: @[@"Configuración del Sistema.app"],
        notes: @[@"Notas"],
        twin: @[@"Calculadora"],
    };
    NSArray<NSString *> *(^names)(NSString *) = ^(NSString *path) {
        return shown[path] ?: @[];
    };
    NSArray *both = @[first, second];
    NSString *decomposed = @"Configuración del Sistema";
    require([applicationPathForDisplayName(@"Calculadora", both, names) isEqualToString:calculator],
        "a later name of a bundle matches, first directory wins");
    require([applicationPathForDisplayName(@"calculadora.APP", both, names) isEqualToString:calculator],
        "case and .app suffix are ignored");
    require([applicationPathForDisplayName(@"Configuracion del sistema", both, names) isEqualToString:settings],
        "accents are ignored");
    require([applicationPathForDisplayName(decomposed, both, names) isEqualToString:settings],
        "decomposed accents match composed ones");
    require(applicationPathForDisplayName(@"Notas", both, names) == nil,
        "only .app bundles are candidates");
    require(applicationPathForDisplayName(@"Calculadora Pro", both, names) == nil,
        "a partial name does not match");
    require(applicationPathForDisplayName(@"Calculadora", @[[root stringByAppendingPathComponent:@"Missing"]], names) == nil,
        "a missing directory is skipped");
    require([applicationPathForDisplayName(@"Calculadora", @[second, first], names) isEqualToString:twin],
        "directory order decides between equal names");
}

static void readsTranslatedBundleNames(NSString *root) {
    NSString *applications = makeDirectory(root, @"Applications");
    NSString *table = makeBundle(applications, @"Calculator.app", @"Calculator");
    writePlist(@{
        @"en": @{@"CFBundleDisplayName": @"Calculator"},
        @"es": @{@"CFBundleDisplayName": @"Calculadora", @"CFBundleName": @"Calculadora"},
    }, [table stringByAppendingPathComponent:@"Contents/Resources/InfoPlist.loctable"]);
    NSString *strings = makeBundle(applications, @"TextEdit.app", @"TextEdit");
    writePlist(@{@"CFBundleDisplayName": @"Editor de Texto"},
        [strings stringByAppendingPathComponent:@"Contents/Resources/es.lproj/InfoPlist.strings"]);
    NSArray *spanish = @[@"es-MX"];
    require([localizedApplicationNames(table, spanish) containsObject:@"Calculadora"],
        "loctable gives the translated name");
    require([localizedApplicationNames(strings, spanish) containsObject:@"Editor de Texto"],
        "InfoPlist.strings gives the translated name");
    require(![localizedApplicationNames(table, @[@"en-US"]) containsObject:@"Calculadora"],
        "another language does not add the Spanish name");
    NSArray<NSString *> *(^names)(NSString *) = ^(NSString *path) {
        return localizedApplicationNames(path, spanish);
    };
    require([applicationPathForDisplayName(@"Calculadora", @[applications], names) isEqualToString:table],
        "launch lookup finds the bundle by its translated loctable name");
    require([applicationPathForDisplayName(@"editor de texto", @[applications], names) isEqualToString:strings],
        "launch lookup finds the bundle by its translated strings name");
}

int main(void) {
    @autoreleasepool {
        NSString *root = temporaryRoot();
        matchesNamesFromAnyDirectory(root);
        readsTranslatedBundleNames(root);
        [[NSFileManager defaultManager] removeItemAtPath:root error:nil];
        puts("launch display name tests passed");
    }
    return 0;
}
