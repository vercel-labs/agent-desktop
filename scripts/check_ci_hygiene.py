#!/usr/bin/env python3
"""Enforces CI workflow efficiency, cache hygiene, and prune safety invariants."""

import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOWS_DIR = ROOT / ".github" / "workflows"


def split_jobs(text):
    """Splits a workflow file's jobs section into a mapping of job id to lines."""
    jobs = {}
    in_jobs = False
    current_job = None
    for lineno, line in enumerate(text.splitlines(), 1):
        if re.match(r"^jobs:\s*$", line):
            in_jobs = True
            current_job = None
            continue
        if in_jobs and re.match(r"^[A-Za-z0-9_-]+:", line):
            in_jobs = False
            current_job = None
            continue
        if not in_jobs:
            continue
        match = re.match(r"^  ([A-Za-z0-9_-]+):\s*$", line)
        if match:
            current_job = match.group(1)
            jobs[current_job] = []
        elif current_job is not None:
            jobs[current_job].append((lineno, line))
    return jobs


def split_steps(job_lines):
    """Groups lines under a job's steps section into individual step blocks."""
    steps = []
    in_steps = False
    current = None
    for lineno, line in job_lines:
        if re.match(r"^    steps:\s*$", line):
            in_steps = True
            current = None
            continue
        if in_steps and re.match(r"^    [A-Za-z0-9_-]+:", line):
            in_steps = False
            current = None
            continue
        if not in_steps:
            continue
        if re.match(r"^      -\s+", line):
            current = [(lineno, line)]
            steps.append(current)
        elif current is not None:
            current.append((lineno, line))
    return steps


def top_level_env(text):
    """Extracts key-value pairs from a workflow's top-level env block."""
    env = {}
    in_env = False
    for line in text.splitlines():
        if re.match(r"^env:\s*$", line):
            in_env = True
            continue
        if in_env:
            if line and not line.startswith(" ") and not line.startswith("#"):
                break
            match = re.match(r"^  ([A-Za-z0-9_]+):\s*(.+?)\s*$", line)
            if match:
                env[match.group(1)] = match.group(2).strip("'\"")
    return env


def strip_comments(lines):
    """Returns non-comment text for a slice of workflow lines."""
    out = []
    for _, line in lines:
        stripped = line.lstrip()
        if stripped.startswith("#"):
            continue
        out.append(line)
    return "\n".join(out)


def check_workflows(workflow_texts):
    """Validates CI hygiene invariants across a dict of workflow filename to text."""
    errors = []

    ci_text = workflow_texts.get("ci.yml")
    if ci_text is None:
        errors.append("ci.yml is missing")
        return errors

    ci_env = top_level_env(ci_text)
    if ci_env.get("CARGO_INCREMENTAL") != "0":
        errors.append("ci.yml top-level env must set CARGO_INCREMENTAL: 0")
    if ci_env.get("CARGO_PROFILE_DEV_DEBUG") != "line-tables-only":
        errors.append(
            "ci.yml top-level env must set CARGO_PROFILE_DEV_DEBUG: line-tables-only"
        )

    for name, text in sorted(workflow_texts.items()):
        jobs = split_jobs(text)

        if name in ("release.yml", "release-dry-run.yml"):
            for job_id, job_lines in jobs.items():
                for step in split_steps(job_lines):
                    body = strip_comments(step)
                    if re.search(r"\buses:\s*actions/cache(?:/(?:restore|save))?@", body):
                        errors.append(
                            f"{name}:{step[0][0]} ({job_id}): release workflows must not use actions/cache"
                        )

        for job_id, job_lines in jobs.items():
            for step in split_steps(job_lines):
                body = strip_comments(step)
                first_line = step[0][0]

                if re.search(r"\buses:\s*actions/cache(?:/(?:restore|save))?@", body):
                    caches_target = bool(re.search(r"(?m)^\s*(?:path:\s*target/?|target/?)\s*$", body))
                    key_lines = [
                        line for _, line in step if re.match(r"^\s*key:\s*", line)
                    ]
                    if caches_target and any(".rs" in k for k in key_lines):
                        errors.append(
                            f"{name}:{first_line} ({job_id}): target/ cache key must not hash *.rs source files"
                        )

                if name in ("ci.yml", "supply-chain.yml") and re.search(
                    r"\bcargo\s+install\b", body
                ):
                    if not re.search(r"cache-hit\s*!=\s*'true'", body):
                        errors.append(
                            f"{name}:{first_line} ({job_id}): cargo install must be guarded by cache-hit != 'true' or replaced with a verified prebuilt binary"
                        )

                if (
                    name == "ci.yml"
                    and job_id == "ffi-panic-guard"
                    and re.search(r"\buses:\s*actions/cache@", body)
                    and ".cargo/registry" in body
                ):
                    errors.append(
                        f"{name}:{first_line} ({job_id}): cargo registry cache must use actions/cache/restore (read-only)"
                    )

    return errors


def check_prune_script(repo_root):
    """Verifies scripts/prune-ci-target-cache.sh keeps external crates and removes workspace artifacts."""
    script = repo_root / "scripts" / "prune-ci-target-cache.sh"
    if not script.is_file():
        return ["scripts/prune-ci-target-cache.sh is missing"]

    with tempfile.TemporaryDirectory(prefix="ad-ci-hygiene-prune-") as tmp:
        target = Path(tmp) / "target"
        keep_files = [
            target / "debug" / "deps" / "libwindows_sys-0123456789abcdef.rlib",
            target / "debug" / "deps" / "libserde-0123456789abcdef.rlib",
            target / "debug" / ".fingerprint" / "windows_sys-0123456789abcdef" / "lib-windows_sys",
            target / "release" / "deps" / "libserde-0123456789abcdef.rlib",
        ]
        drop_files = [
            target / "debug" / "deps" / "libagent_desktop_core-0123456789abcdef.rlib",
            target / "debug" / "deps" / "windows_private_file_install-0123456789abcdef",
            target / "debug" / ".fingerprint" / "agent_desktop_core-0123456789abcdef" / "lib",
            target / "debug" / "incremental" / "crate" / "dep-graph.bin",
            target / "release" / "agent-desktop",
            target / "release-ffi" / "libagent_desktop_ffi.dylib",
        ]
        for path in keep_files + drop_files:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"fixture")

        proc = subprocess.run(
            [shutil.which("bash") or "bash", script.relative_to(repo_root).as_posix(), target.as_posix()],
            cwd=str(repo_root),
            capture_output=True,
            text=True,
            check=False,
        )
        if proc.returncode != 0:
            return [f"prune-ci-target-cache.sh failed on fixture: {proc.stderr.strip()}"]

        errors = []
        for path in keep_files:
            if not path.exists():
                errors.append(f"prune-ci-target-cache.sh deleted external artifact {path.relative_to(target)}")
        for path in drop_files:
            if path.exists():
                errors.append(f"prune-ci-target-cache.sh retained workspace artifact {path.relative_to(target)}")
        return errors


def self_test():
    """Proves every workflow rule passes a clean fixture and rejects planted violations."""
    clean_ci = """name: CI
env:
  CARGO_INCREMENTAL: 0
  CARGO_PROFILE_DEV_DEBUG: line-tables-only
jobs:
  test:
    steps:
      - uses: actions/cache/restore@deadbeef
        with:
          path: target/
          key: ${{ runner.os }}-deps-${{ hashFiles('Cargo.lock') }}
  ffi-panic-guard:
    steps:
      - uses: actions/cache/restore@deadbeef
        with:
          path: ~/.cargo/registry/cache/
          key: ${{ runner.os }}-cargo-${{ hashFiles('Cargo.lock') }}
      - id: tool
        uses: actions/cache@deadbeef
        with:
          path: ~/.cargo/bin/cbindgen
          key: macOS-ARM64-cbindgen-0.29.4
      - if: steps.tool.outputs.cache-hit != 'true'
        run: cargo install cbindgen --version 0.29.4 --locked
"""
    clean_release = """name: Release
jobs:
  build:
    steps:
      - run: cargo build --release
"""
    clean_sc = """name: Supply Chain
jobs:
  audit:
    steps:
      - run: echo ok
"""
    base = {
        "ci.yml": clean_ci,
        "release.yml": clean_release,
        "release-dry-run.yml": clean_release,
        "supply-chain.yml": clean_sc,
    }
    clean_errors = check_workflows(base)
    if clean_errors:
        raise AssertionError(f"clean fixture failed: {clean_errors}")

    plants = [
        (
            "missing CARGO_INCREMENTAL",
            {**base, "ci.yml": clean_ci.replace("  CARGO_INCREMENTAL: 0\n", "")},
        ),
        (
            "missing CARGO_PROFILE_DEV_DEBUG",
            {
                **base,
                "ci.yml": clean_ci.replace(
                    "  CARGO_PROFILE_DEV_DEBUG: line-tables-only\n", ""
                ),
            },
        ),
        (
            "source-hashed target cache",
            {
                **base,
                "ci.yml": clean_ci.replace(
                    "hashFiles('Cargo.lock')",
                    "hashFiles('Cargo.lock') }}-${{ hashFiles('crates/**/*.rs')",
                ),
            },
        ),
        (
            "cache in release.yml",
            {
                **base,
                "release.yml": clean_release
                + "      - uses: actions/cache@deadbeef\n        with:\n          path: target/\n          key: bad\n",
            },
        ),
        (
            "unguarded cargo install",
            {
                **base,
                "supply-chain.yml": clean_sc
                + "      - run: cargo install --locked cargo-deny --version 0.20.2\n",
            },
        ),
        (
            "read-write ffi registry cache",
            {
                **base,
                "ci.yml": clean_ci.replace(
                    "      - uses: actions/cache/restore@deadbeef\n        with:\n          path: ~/.cargo/registry/cache/",
                    "      - uses: actions/cache@deadbeef\n        with:\n          path: ~/.cargo/registry/cache/",
                ),
            },
        ),
    ]
    for label, mutated in plants:
        if not check_workflows(mutated):
            raise AssertionError(f"self-test failed to reject planted violation: {label}")


def main():
    """Runs the self-test, workflow checks, and target-prune fixture verification."""
    self_test()
    workflow_texts = {
        path.name: path.read_text(encoding="utf-8")
        for path in sorted(WORKFLOWS_DIR.glob("*.yml"))
    }
    errors = check_workflows(workflow_texts) + check_prune_script(ROOT)
    if errors:
        for error in errors:
            print(f"FAIL: {error}", file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: CI hygiene checks passed across {len(workflow_texts)} workflow(s)")


if __name__ == "__main__":
    main()
