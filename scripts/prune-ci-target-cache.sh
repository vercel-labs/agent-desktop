#!/usr/bin/env bash
#
# Prunes a cargo target directory down to the compiled third-party
# dependencies that a later run can reuse.
#
# A restored cache only pays for its download when everything in it is reused.
# Incremental state, example and test binaries, workspace rlibs, top-level
# executables and dSYM trees are rebuilt on every run anyway, and they are what
# makes a warm target directory outgrow the cache. Third-party rlibs, rmeta
# files, build-script output and their fingerprints stay, because those are the
# artifacts a fresh checkout cannot cheaply reproduce.
#
# Entries whose crate Cargo.lock resolves but the workspace does not own are
# never removed. That keeps a workspace artifact prefix honest: `windows_*`
# also spells the windows-* dependency family, and deleting those rlibs would
# throw away the most expensive work the cache exists to carry.
#
# Usage: scripts/prune-ci-target-cache.sh [target-dir]

set -euo pipefail

repo_root=$(cd "$(dirname "$0")/.." && pwd)
target_dir=${1:-${CARGO_TARGET_DIR:-target}}

if [ ! -d "$target_dir" ]; then
    exit 0
fi

external_list="|"
if [ -f "$repo_root/Cargo.lock" ]; then
    while IFS= read -r crate_name; do
        [ -n "$crate_name" ] || continue
        external_list="${external_list}${crate_name}|"
    done < <(
        sed -n 's/^name = "\(.*\)"$/\1/p' "$repo_root/Cargo.lock" \
            | tr '-' '_' \
            | grep -v '^agent_desktop' \
            | sort -u || true
    )
fi

is_external_crate() {
    case "$external_list" in
        *"|$1|"*) return 0 ;;
        *) return 1 ;;
    esac
}

remove_entry() {
    if [ -e "$1" ] || [ -L "$1" ]; then
        rm -rf "$1"
    fi
}

artifact_stem() {
    local stem
    stem=${1##*/}
    stem=${stem%.*}
    stem=${stem#lib}
    printf '%s\n' "${stem%-*}"
}

prune_workspace_artifacts() {
    local directory entry stem
    directory=$1
    if [ ! -d "$directory" ]; then
        return 0
    fi
    for entry in "$directory"/*; do
        if [ ! -e "$entry" ] && [ ! -L "$entry" ]; then
            continue
        fi
        case "${entry##*/}" in
            agent_desktop* | libagent_desktop* | agent-desktop* | c_abi_* | c_header_* | cpp_header_* | error_lifetime* | cli_* | mcp_* | command_* | snapshot_* | windows_* | retained_fixture* | locator_benchmark* | uia_tree_dump* | ax_probe*) ;;
            *) continue ;;
        esac
        stem=${entry##*/}
        stem=${stem%.*}
        stem=${stem#lib}
        stem=${stem%-*}
        if is_external_crate "$stem"; then
            continue
        fi
        remove_entry "$entry"
    done
}

for profile_dir in "$target_dir"/*; do
    if [ ! -d "$profile_dir" ]; then
        continue
    fi
    remove_entry "$profile_dir/incremental"
    remove_entry "$profile_dir/examples"
    remove_entry "$profile_dir/tmp"
    prune_workspace_artifacts "$profile_dir/deps"
    prune_workspace_artifacts "$profile_dir/.fingerprint"
done

remove_entry "$target_dir/tmp"

for profile in debug release release-ffi; do
    profile_dir="$target_dir/$profile"
    if [ ! -d "$profile_dir" ]; then
        continue
    fi
    for entry in "$profile_dir"/*; do
        if [ ! -e "$entry" ] && [ ! -L "$entry" ]; then
            continue
        fi
        case "${entry##*/}" in
            agent-desktop* | libagent_desktop* | *.dSYM) remove_entry "$entry" ;;
        esac
    done
done

for entry in "$target_dir"/debug/deps/*.dSYM; do
    remove_entry "$entry"
done
