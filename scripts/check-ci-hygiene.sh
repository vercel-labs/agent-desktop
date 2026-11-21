#!/usr/bin/env bash
set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

INTERPRETER_CANDIDATES=(python3 python py)

resolve_interpreter() {
    local candidate
    for candidate in "${INTERPRETER_CANDIDATES[@]}"; do
        if command -v "$candidate" >/dev/null 2>&1 &&
            "$candidate" -c 'import sys; raise SystemExit(0 if sys.version_info[0] >= 3 else 1)' \
                >/dev/null 2>&1; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    return 1
}

if ! interpreter="$(resolve_interpreter)"; then
    printf 'no working Python 3 interpreter on PATH (tried python3, python, py)\n' >&2
    exit 1
fi

"$interpreter" scripts/check_ci_hygiene.py "$@"
