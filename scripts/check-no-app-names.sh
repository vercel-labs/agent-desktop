#!/usr/bin/env bash
#
# Fails when shipped runtime source names or matches a specific application.
#
# agent-desktop must act the same way in every application, so runtime code
# decides from roles, capabilities, ownership and process identity, never from
# an application's name, bundle id, desktop id, executable or window title.
# Application names belong in tests and examples only.
#
#   banned:  if app.name == "Finder" { ... }
#   banned:  const TARGET: &str = "com.apple.Safari";
#   banned:  launch("notepad.exe")
#   fine:    the same literal in a *_tests.rs file, a test-only module, or a
#            #[cfg(test)] item
#
# Scope is *.rs under crates/ and src/, minus test-only files: *_tests.rs,
# tests/, *_tests/, test_support, examples/ and benches/ paths, files whose
# first line is #![cfg(test)], files declared by `mod name;` right after
# #[cfg(test)], and everything those files declare in turn. Inside a runtime
# file the single item following each #[cfg(test)] attribute - an inline module
# or other braced item, or one item ended by `;` - is skipped; code on the same
# line after the item is scanned. A file that any runtime module also declares
# is scanned.
#
# Three rules fire on the remaining lines:
#   1. a string literal equal to an application name or id installed on this
#      machine, plus a committed floor list for bare CI runners. A Linux
#      desktop file id counts only when it is a reverse-DNS id: a bare one
#      such as "code" is an ordinary word
#   2. identity shapes that name any app: "<name>.exe", "<Name>.app",
#      "<id>.desktop" or a Windows package family name
#   3. an identity field (.name, .bundle_id, .title, ...) compared to a
#      non-empty string literal, with the literal on either side
#
# Platform contracts are not applications and are listed in ALLOWED, which
# changes only in review. An allowed literal is blanked, not the whole line, so
# a forbidden name on the same line still fires: the Dock and menu bar agent bundle ids, the GNOME
# Shell/Mutter and KDE StatusNotifier D-Bus names (platform surfaces the
# adapters must name), flag("Terminal") (the Desktop Entry Terminal key),
# FRAMEWORK_ID "Chrome" (the UIA framework id Windows reports for a toolkit),
# call_method("Screenshot") (the xdg-desktop-portal method), "NotificationCenter"
# (the macOS process the notifications module signals), ShellKind::Dock => "Dock"
# (an output label), the PROTECTED_PROCESSES lists (OS-critical processes
# close-app refuses: refusal only, never behavior; to be replaced by a platform
# signal), "target": "System Events" and the osascript line naming System Events
# and ControlCenter (the Automation permission probe and notification click
# target platform processes by contract), and whose description is "Clock"
# (TRACKED DEFECT: English UI label; to be replaced by the clock menu extra's
# non-localized identifier).

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

OS="$(uname -s)"

# The Dock and menu-bar agent bundle ids, the GNOME Shell/Mutter and KDE
# StatusNotifier D-Bus names: platform surfaces the adapters must name.
# flag("Terminal"): the Desktop Entry Terminal key is a platform field.
# FRAMEWORK_ID "Chrome": the UIA framework id Windows reports for a toolkit.
# call_method("Screenshot"): the xdg-desktop-portal Screenshot method name.
# "NotificationCenter": the macOS process the notifications module signals.
# ShellKind::Dock => "Dock": an output label for the dock surface.
# PROTECTED_PROCESSES: OS-critical processes close-app refuses - refusal only,
# never behavior; to be replaced by a platform signal.
# "target": "System Events" and the osascript line: the Automation permission
# probe and the notification click target platform processes by contract.
# whose description is "Clock": TRACKED DEFECT: English UI label; to be
# replaced by the clock menu extra's non-localized identifier.
ALLOWED='"(com\.apple\.(dock|MenuBarAgent|controlcenter|systemuiserver|systemevents)|org\.gnome\.(Shell|Mutter)(\.[A-Za-z.]+)?|org\.kde\.StatusNotifier(Item|Watcher))"|flag\("Terminal"\)|FRAMEWORK_ID: &str = "Chrome"|call_method\("Screenshot"|"NotificationCenter"|ShellKind::Dock => "Dock"|&\["loginwindow", "windowserver", "dock", "launchd", "finder"\]|"(csrss|wininit|winlogon|services|lsass|smss|lsaiso|dwm|explorer)\.exe"|"target": "System Events"|tell application "System Events" to tell its application process "ControlCenter"|whose description is \\?"Clock\\?"'

FIELDS='app_name|bundle_id|window_title|process_name|image_name|desktop_id|class_name|wm_class|app_id|executable|exe|image|name|app|title'
RULE2='"[^"{}]+\.[Ee][Xx][Ee]"|"[A-Z][^"{}]*\.app"|"[A-Za-z0-9_.-]+\.desktop"|"[A-Za-z0-9]+(\.[A-Za-z0-9]+)+_[a-z0-9]{13}(![A-Za-z0-9_.]+)?"'
RULE3="\\.(${FIELDS})[[:space:]]*(==|!=)[[:space:]]*\"[^\"]+\"|\\.(${FIELDS})[[:space:]]*==[[:space:]]*Some[[:space:]]*[(][[:space:]]*\"[^\"]+\"[[:space:]]*[)][[:space:]]*|\\.(${FIELDS})([(][)]|\\.as_deref[(][)]|\\.as_str[(][)])?[[:space:]]*\\.(eq|eq_ignore_ascii_case|contains|starts_with|ends_with)[[:space:]]*[(][[:space:]]*\"[^\"]+\"|(Some[[:space:]]*[(][[:space:]]*)?\"[^\"]+\"([[:space:]]*[)])?[[:space:]]*(==|!=)[[:space:]]*(Some[[:space:]]*[(][[:space:]]*)?&?[A-Za-z_][A-Za-z0-9_]*(\\.[A-Za-z_][A-Za-z0-9_]*)*\\.(${FIELDS})([(][)])?(\\.as_deref[(][)]|\\.as_str[(][)])?([^A-Za-z0-9_(]|$)"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/agent-desktop-app-names.XXXXXXXX")"
trap 'rm -rf "$WORK"' EXIT

cat > "$WORK/classify.awk" <<'AWK_EOF'
function dirname_of(p,   d) {
    d = p
    if (sub(/\/[^\/]*$/, "", d) == 0) return "."
    if (d == "") return "/"
    return d
}
function basename_of(p,   b) {
    b = p
    sub(/^.*\//, "", b)
    return b
}
function normalize(p,   n, parts, i, out, isabs) {
    isabs = (substr(p, 1, 1) == "/")
    n = split(p, parts, "/")
    out = ""
    for (i = 1; i <= n; i++) {
        if (parts[i] == "" || parts[i] == ".") continue
        if (parts[i] == "..") {
            if (out ~ /\//) sub(/\/[^\/]*$/, "", out)
            else out = ""
            continue
        }
        out = (out == "" ? parts[i] : out "/" parts[i])
    }
    return (isabs ? "/" : "") out
}
function file_exists(p,   line) {
    if ((getline line < p) >= 0) { close(p); return 1 }
    close(p); return 0
}
function test_path(p) {
    return (p ~ /_tests\.rs$/ || p ~ /\/tests\// || p ~ /_tests\// || \
        p ~ /test_support/ || p ~ /\/examples\// || p ~ /\/benches\//)
}
{
    F = normalize($0)
    forder[++fcount] = F
    n = 0
    while ((getline line < F) > 0) L[++n] = line
    close(F)
    if (test_path(F)) testonly[F] = 1
    first = ""
    for (i = 1; i <= n; i++) {
        if (L[i] !~ /^[[:space:]]*$/) { first = L[i]; break }
    }
    if (first ~ /^[[:space:]]*#![[:space:]]*\[[[:space:]]*cfg[[:space:]]*\([[:space:]]*test[[:space:]]*\)[[:space:]]*\]/) {
        testonly[F] = 1
    }
    for (i = 1; i <= n; i++) {
        if (L[i] !~ /^[[:space:]]*(pub([[:space:]]*[(][^)]*[)])?[[:space:]]+)?mod[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[[:space:]]*;[[:space:]]*(\/\/.*)?$/) continue
        if (match(L[i], /mod[[:space:]]+[A-Za-z_][A-Za-z0-9_]*/) == 0) continue
        seg = substr(L[i], RSTART, RLENGTH)
        nn = split(seg, tok, /[[:space:]]+/)
        mname = tok[nn]
        gated = 0
        pattr = ""
        for (j = i - 1; j >= 1; j--) {
            l = L[j]
            if (l ~ /^[[:space:]]*$/) continue
            if (l ~ /^[[:space:]]*\/\//) continue
            if (l ~ /^[[:space:]]*#!?\[/) {
                if (l ~ /^[[:space:]]*#!?\[[[:space:]]*cfg[[:space:]]*\([[:space:]]*test[[:space:]]*\)/) gated = 1
                if (pattr == "" && l ~ /^[[:space:]]*#!?\[[[:space:]]*path[[:space:]]*=/) {
                    if (match(l, /"[^"]*"/) > 0) pattr = substr(l, RSTART + 1, RLENGTH - 2)
                }
                continue
            }
            break
        }
        dir = dirname_of(F)
        base = basename_of(F)
        if (pattr != "") {
            cand = dir "/" pattr
            if (file_exists(cand)) {
                eto[++ne] = normalize(cand)
                efrom[ne] = F
                egate[ne] = gated
            }
        } else {
            if (base == "mod.rs" || base == "lib.rs" || base == "main.rs") {
                c1 = dir "/" mname ".rs"
                c2 = dir "/" mname "/mod.rs"
            } else {
                stem = base
                sub(/\.rs$/, "", stem)
                c1 = dir "/" stem "/" mname ".rs"
                c2 = dir "/" stem "/" mname "/mod.rs"
            }
            if (file_exists(c1)) {
                eto[++ne] = normalize(c1)
                efrom[ne] = F
                egate[ne] = gated
            } else if (file_exists(c2)) {
                eto[++ne] = normalize(c2)
                efrom[ne] = F
                egate[ne] = gated
            }
        }
    }
}
END {
    changed = 1
    while (changed) {
        changed = 0
        delete runtime_edges
        for (k = 1; k <= ne; k++) {
            if (!(egate[k] == 1 || (efrom[k] in testonly))) runtime_edges[eto[k]]++
        }
        for (k = 1; k <= ne; k++) {
            if (!(eto[k] in testonly) && !(eto[k] in runtime_edges)) {
                testonly[eto[k]] = 1
                changed = 1
            }
        }
    }
    for (k = 1; k <= fcount; k++) {
        if (!(forder[k] in testonly)) print forder[k]
    }
}
AWK_EOF

cat > "$WORK/scan.awk" <<'AWK_EOF'
function trim(s) {
    gsub(/^[[:space:]]+/, "", s)
    gsub(/[[:space:]]+$/, "", s)
    return s
}
function strip_comments(s,   out, rest, tok) {
    out = ""
    rest = s
    while (match(rest, /"(\\.|[^"\\])*"|\/\//) > 0) {
        tok = substr(rest, RSTART, RLENGTH)
        if (tok == "//") {
            out = out substr(rest, 1, RSTART - 1)
            return out
        }
        out = out substr(rest, 1, RSTART - 1) tok
        rest = substr(rest, RSTART + RLENGTH)
    }
    return out rest
}
function is_cfgtest(s) {
    return (s ~ /^[[:space:]]*#!?\[[[:space:]]*cfg[[:space:]]*\([[:space:]]*test[[:space:]]*\)[[:space:]]*\]/)
}
function skip_attribute(s, i, n,   d, c) {
    d = 0
    for (; i <= n; i++) {
        c = substr(s, i, 1)
        if (c == "[") d++
        else if (c == "]") {
            d--
            if (d <= 0) return i + 1
        }
    }
    return n + 1
}
function string_end(s, i, n, h,   c, k, ok) {
    for (; i <= n; i++) {
        c = substr(s, i, 1)
        if (h < 0 && c == "\\") { i++; continue }
        if (c != "\"") continue
        ok = 1
        if (h < 0) return i + 1
        for (k = 1; k <= h; k++) if (substr(s, i + k, 1) != "#") { ok = 0; break }
        if (ok) return i + h + 1
    }
    return 0
}
function raw_hashes(s, i,   j, p) {
    j = i - 1
    while (j >= 1 && substr(s, j, 1) == "#") j--
    if (j < 1 || substr(s, j, 1) != "r") return -1
    p = substr(s, j - 1, 1)
    if (j > 1 && p != "b" && p ~ /[A-Za-z0-9_]/) return -1
    return i - 1 - j
}
function consume(s,   i, n, c, rest, e) {
    n = length(s)
    i = 1
    while (i <= n) {
        if (instr) {
            e = string_end(s, i, n, strh)
            if (e == 0) return ""
            instr = 0
            i = e
            continue
        }
        c = substr(s, i, 1)
        if (c == "\"") {
            strh = raw_hashes(s, i)
            instr = 1
            i++
            continue
        }
        if (c == "'") {
            rest = substr(s, i)
            if (match(rest, /^'(\\.|[^'\\])'/) > 0) { i += RLENGTH; continue }
            i++
            continue
        }
        if (c ~ /[[:space:]]/) { i++; continue }
        if (c == "#" && !seen) { i = skip_attribute(s, i, n); continue }
        seen = 1
        if (c == "(" || c == "[") pd++
        else if (c == ")" || c == "]") { if (pd > 0) pd-- }
        else if (c == "{") { bd++; st = 2 }
        else if (c == "}") {
            if (bd == 0) { st = 0; return substr(s, i) }
            bd--
            if (bd == 0) { st = 0; return substr(s, i + 1) }
        }
        else if (c == ";" && bd == 0 && pd == 0) { st = 0; return substr(s, i + 1) }
        i++
    }
    return ""
}
function begin_skip(s) {
    st = 1
    bd = 0
    pd = 0
    seen = 0
    return consume(substr(s, index(s, "]") + 1))
}
{
    F = $0
    n = 0
    while ((getline line < F) > 0) L[++n] = line
    close(F)
    st = 0
    instr = 0
    for (i = 1; i <= n; i++) {
        seg = L[i]
        if (index(seg, "//") > 0) seg = strip_comments(seg)
        if (st != 0) seg = consume(seg)
        while (st == 0 && is_cfgtest(seg)) seg = begin_skip(seg)
        if (st == 0 && trim(seg) != "") print F ":" i ":" seg
    }
}
AWK_EOF

emit_app() {
    local app="$1" base plist
    base="${app##*/}"
    base="${base%.app}"
    printf '%s\n' "$base"
    plist="$app/Contents/Info.plist"
    [ -f "$plist" ] && command -v plutil >/dev/null 2>&1 || return 0
    plutil -extract CFBundleName raw -o - "$plist" 2>/dev/null || true
    plutil -extract CFBundleDisplayName raw -o - "$plist" 2>/dev/null || true
    plutil -extract CFBundleIdentifier raw -o - "$plist" 2>/dev/null || true
}

scan_app_dir() {
    local app
    for app in "$1"/*.app; do
        [ -d "$app" ] || continue
        emit_app "$app"
    done
}

collect_macos_apps() {
    local dir sub
    for dir in /Applications /System/Applications /System/Applications/Utilities \
               /System/Library/CoreServices "$HOME/Applications"; do
        [ -d "$dir" ] || continue
        scan_app_dir "$dir"
        if [ "$dir" = "/Applications" ]; then
            for sub in "$dir"/*/; do
                [ -d "$sub" ] || continue
                scan_app_dir "$sub"
            done
        fi
    done
}

desk_name() {
    awk '
        /^\[Desktop Entry\]/ { insec = 1; next }
        /^\[/ { insec = 0 }
        insec == 1 && /^Name=/ {
            sub(/^Name=/, "")
            gsub(/^[[:space:]]+|[[:space:]]+$/, "")
            if ($0 != "") print
            exit
        }
    ' "$1" 2>/dev/null
}

collect_linux_apps() {
    local base appdir f id name dirs
    dirs="${XDG_DATA_DIRS:-/usr/local/share:/usr/share}:/var/lib/flatpak/exports/share"
    if [ -n "${XDG_DATA_HOME:-}" ]; then
        dirs="$XDG_DATA_HOME:$dirs"
    elif [ -n "${HOME:-}" ]; then
        dirs="$HOME/.local/share:$dirs"
    fi
    local IFS=':'
    for base in $dirs; do
        appdir="$base/applications"
        [ -d "$appdir" ] || continue
        for f in "$appdir"/*.desktop; do
            [ -f "$f" ] || continue
            id="${f##*/}"
            id="${id%.desktop}"
            case "$id" in
                *.*) printf '%s\n' "$id" ;;
            esac
            name="$(desk_name "$f")"
            [ -n "$name" ] && printf '%s\n' "$name"
        done
    done
}

collect_windows_apps() {
    command -v powershell.exe >/dev/null 2>&1 || return 0
    # shellcheck disable=SC2016
    powershell.exe -NoProfile -Command 'Get-StartApps | ForEach-Object { $_.Name; $_.AppID }; Get-AppxPackage | ForEach-Object { $_.Name; $_.PackageFamilyName }' 2>/dev/null | tr -d '\r' || true
}

collect_app_names() {
    printf '%s\n' \
        'Finder' 'Safari' 'Xcode' 'TextEdit' 'Preview' 'Calculator' \
        'Notepad' 'WordPad' 'Paint' 'Explorer' 'Slack' 'Google Chrome' \
        'Firefox' 'Visual Studio Code' 'gedit' 'Nautilus' 'nautilus' \
        'kate' 'kcalc' 'Text Editor' 'System Settings' 'Mail' 'Calendar' \
        'Character Map' 'Stickies' \
        'com.apple.finder' 'com.apple.Safari' 'com.apple.TextEdit' \
        'com.apple.dt.Xcode' 'com.google.Chrome' \
        'com.tinyspeck.slackmacgap' 'org.gnome.Nautilus' \
        'org.gnome.TextEditor' 'org.gnome.gedit' 'org.kde.kate'
    case "$OS" in
        Darwin) collect_macos_apps ;;
        Linux) collect_linux_apps ;;
        MINGW*|MSYS*|CYGWIN*) collect_windows_apps ;;
    esac
}

runtime_lines() {
    { find "$@" -name '*.rs' -print 2>/dev/null || true; } | LC_ALL=C sort |
        awk -f "$WORK/classify.awk" |
        awk -f "$WORK/scan.awk"
}

hit_lines() {
    local clean="$WORK/clean.txt" tags="$WORK/tags.txt"
    sed -E "s#${ALLOWED}#@#g" "$1" > "$clean"
    {
        { grep -n -F -f "$WORK/patterns.txt" "$clean" 2>/dev/null || true; } |
            sed 's/:.*$/ rule 1: names an application/'
        { grep -nE "$RULE2" "$clean" 2>/dev/null || true; } |
            sed 's/:.*$/ rule 2: application identity/'
        { grep -nE "$RULE3" "$clean" 2>/dev/null || true; } |
            sed 's/:.*$/ rule 3: identity field comparison/'
    } > "$tags"
    awk '
        FILENAME == ARGV[1] { n = index($0, " "); m[++c] = substr($0, 1, n - 1); t[c] = substr($0, n + 1); need[m[c]] = 1; next }
        FNR in need { L[FNR] = $0 }
        END { for (k = 1; k <= c; k++) print L[m[k]] "  [" t[k] "]" }
    ' "$tags" "$1" | LC_ALL=C sort -u
}

cleanup_fixture_tree() {
    if command -v trash >/dev/null 2>&1; then
        trash "$1"
    else
        printf 'Cleanup retained %s: trash is unavailable\n' "$1" >&2
    fi
}

self_test() {
    local root hits failures=0
    root="$WORK/selftest"
    mkdir -p "$root/crates/x/src"
    printf 'const N: &str = "Finder";\n' > "$root/crates/x/src/named.rs"
    printf 'const E: &str = "Foo.exe";\n' > "$root/crates/x/src/exe.rs"
    printf 'fn f(app: &App) -> bool { app.name == "Anything" }\n' > "$root/crates/x/src/cmp.rs"
    printf 'const D: &str = "com.apple.dock";\n' > "$root/crates/x/src/allowed.rs"
    printf 'const P: &str = "clean";\n#[cfg(test)]\n#[path = "x_live.rs"]\nmod live;\n' > "$root/crates/x/src/live_parent.rs"
    printf 'const L: &str = "Finder";\n#[path = "x_live_child.rs"]\nmod child;\n' > "$root/crates/x/src/x_live.rs"
    printf 'const C: &str = "Finder";\n' > "$root/crates/x/src/x_live_child.rs"
    printf 'const BEFORE: &str = "ok";\n#[cfg(test)]\nmod tests {\n    const T: &str = "Finder";\n}\nconst AFTER: &str = "Finder";\n' > "$root/crates/x/src/inline.rs"
    printf 'const A: &str = "com.apple.dock"; const MIXED: &str = "Finder";\n' > "$root/crates/x/src/mixed.rs"
    printf '#[path = "x_both.rs"]\nmod both;\n' > "$root/crates/x/src/both_runtime.rs"
    printf '#[cfg(test)]\n#[path = "x_both.rs"]\nmod both;\n' > "$root/crates/x/src/both_gated.rs"
    printf 'const BOTH: &str = "Finder";\n' > "$root/crates/x/src/x_both.rs"
    printf '#[cfg(test)] mod t { const T: &str = "Finder"; } const SAMELINE: &str = "Finder";\n' > "$root/crates/x/src/sameline.rs"
    printf '#[cfg(test)]\nmod t {\n    const T: &str = "Finder";\n} const CLOSEDLINE: &str = "Finder";\n' > "$root/crates/x/src/closedline.rs"
    printf 'fn f(app: &App) -> bool { "Anything" == app.name }\n' > "$root/crates/x/src/reversed.rs"
    printf '#[cfg(test)]\nconst T: &str = "line one\nline two";\nconst MULTILINE: &str = "Finder";\n' > "$root/crates/x/src/multiline.rs"
    printf '#[cfg(test)]\nconst T: &str = r#"line "one"\nline two"#;\nconst RAWLINE: &str = "Finder";\n' > "$root/crates/x/src/rawline.rs"
    runtime_lines "$root/crates" > "$WORK/selftest_scan.txt"
    hits="$(hit_lines "$WORK/selftest_scan.txt")"
    if ! printf '%s\n' "$hits" | grep -q 'named\.rs'; then
        printf 'self-test FAIL (missed): a floor-list application name\n' >&2
        failures=1
    fi
    if ! printf '%s\n' "$hits" | grep -q 'exe\.rs'; then
        printf 'self-test FAIL (missed): an .exe identity literal\n' >&2
        failures=1
    fi
    if ! printf '%s\n' "$hits" | grep -q 'cmp\.rs'; then
        printf 'self-test FAIL (missed): an identity field comparison\n' >&2
        failures=1
    fi
    if [ "$(printf '%s\n' "$hits" | grep -c 'inline\.rs' || true)" != "1" ] || \
       ! printf '%s\n' "$hits" | grep 'inline\.rs' | grep -q 'AFTER'; then
        printf 'self-test FAIL: code after an inline #[cfg(test)] block must still be scanned\n' >&2
        failures=1
    fi
    if ! printf '%s\n' "$hits" | grep 'mixed\.rs' | grep -q 'MIXED'; then
        printf 'self-test FAIL (missed): a forbidden name on a line that also holds an allowed literal\n' >&2
        failures=1
    fi
    if ! printf '%s\n' "$hits" | grep -q 'x_both\.rs'; then
        printf 'self-test FAIL (missed): a file declared by a runtime module and by a #[cfg(test)] module\n' >&2
        failures=1
    fi
    if ! printf '%s\n' "$hits" | grep 'sameline\.rs' | grep -q 'SAMELINE' || \
       printf '%s\n' "$hits" | grep 'sameline\.rs' | grep -q 'const T:'; then
        printf 'self-test FAIL: code after a one-line #[cfg(test)] item must be scanned and the item skipped\n' >&2
        failures=1
    fi
    if ! printf '%s\n' "$hits" | grep 'closedline\.rs' | grep -q 'CLOSEDLINE' || \
       printf '%s\n' "$hits" | grep 'closedline\.rs' | grep -q 'const T:'; then
        printf 'self-test FAIL: code after the closing brace of a #[cfg(test)] item must be scanned\n' >&2
        failures=1
    fi
    if ! printf '%s\n' "$hits" | grep 'multiline\.rs' | grep -q 'MULTILINE' || \
       ! printf '%s\n' "$hits" | grep 'rawline\.rs' | grep -q 'RAWLINE'; then
        printf 'self-test FAIL (missed): code after a #[cfg(test)] item whose string literal spans lines\n' >&2
        failures=1
    fi
    if ! printf '%s\n' "$hits" | grep -q 'reversed\.rs'; then
        printf 'self-test FAIL (missed): a string literal compared to an identity field\n' >&2
        failures=1
    fi
    if printf '%s\n' "$hits" | grep -qE 'live_parent\.rs|x_live\.rs|x_live_child\.rs|allowed\.rs'; then
        printf 'self-test FAIL (false positive): a test-only or allowed line\n' >&2
        failures=1
    fi
    cleanup_fixture_tree "$root"
    return "$failures"
}

failed=0

collect_app_names |
    awk 'length($0) >= 3 && $0 !~ /"/ && $0 !~ /^[[:space:]]*$/ { printf "\"%s\"\n", $0 }' |
    LC_ALL=C sort -u > "$WORK/patterns.txt"

self_test || failed=1

runtime_lines crates src > "$WORK/scan.txt"
hits="$(hit_lines "$WORK/scan.txt")"
if [ -n "$hits" ]; then
    printf '%s\n' "$hits" >&2
    printf '  ^ runtime code names an application - decide from roles, capabilities, ownership or process identity\n' >&2
    failed=1
fi

exit "$failed"
