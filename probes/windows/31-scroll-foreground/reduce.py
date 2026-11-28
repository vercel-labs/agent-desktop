#!/usr/bin/env python3
"""Reduces the raw probe captures to bounded, identity-free captures and to the markdown tables.

usage: reduce.py <raw root> <captures dir> <tables.md>
The raw root holds out-a (default client), out-b (AutoSetFocus off), out-a5 (default, with ScrollItem),
out-act (other pattern calls) and out-partb (shipped commands).
"""
import collections
import glob
import json
import os
import sys

OUTSIDE = ["fg", "fg_active", "fg_focus", "f_active", "f_focus", "uia_focus", "t_iconic", "t_visible", "t_above_f", "raised_past"]
ORDER = ["win32", "winforms", "wpf", "directui", "xaml", "chromium", "chromium-div"]
CALLS = ["positive_control_SetFocus", "negative_control_CurrentName", "A1_Scroll", "A2_SetScrollPercent",
         "A5_ScrollIntoView", "A3_FindItemByProperty", "A4_Realize"]
SHORT = {"positive_control_SetFocus": "control+ SetFocus", "negative_control_CurrentName": "control- CurrentName",
         "A1_Scroll": "A1 Scroll", "A2_SetScrollPercent": "A2 SetScrollPercent", "A5_ScrollIntoView": "A5 ScrollIntoView",
         "A3_FindItemByProperty": "A3 FindItemByProperty", "A4_Realize": "A4 Realize"}


def any_of(row, fields):
    return any(f in row["changed_at_any_time"] for f in fields)


def cell(rows):
    real = [r for r in rows if "skipped" not in r]
    if not real:
        return {"trials": 0, "skipped": rows[0]["skipped"]}
    hres = collections.Counter(r["outcome"]["hresult"] or "crate sentinel %s" % r["outcome"]["crate_sentinel"] for r in real)
    percents = [(r["before"]["v_percent"], r["after"]["v_percent"]) for r in real]
    return {
        "trials": len(real),
        "hresult": dict(hres),
        "percent_first": percents[0][0],
        "percent_last": percents[-1][1],
        "moved": sum(1 for b, a in percents if b is not None and a is not None and abs(a - b) > 1e-9),
        "foreground": sum(1 for r in real if any_of(r, ["fg"])),
        "focus_hwnd": sum(1 for r in real if any_of(r, ["fg_focus", "f_focus"])),
        "uia_focus": sum(1 for r in real if any_of(r, ["uia_focus"])),
        "t_thread": sum(1 for r in real if any_of(r, ["t_active", "t_focus"])),
        "t_above_f": sum(1 for r in real if any_of(r, ["t_above_f"])),
        "raised": sum(1 for r in real if any_of(r, ["raised_past"])),
        "iconic": sum(1 for r in real if any_of(r, ["t_iconic"])),
        "any_outside": sum(1 for r in real if any_of(r, OUTSIDE)),
        "winevents": {"foreground": sum(r["winevents"]["foreground"] for r in real),
                      "object_focus": sum(r["winevents"]["object_focus"] for r in real),
                      "minimize": sum(r["winevents"]["minimize_start"] + r["winevents"]["minimize_end"] for r in real)},
        "precondition_failed": sum(1 for r in real if not r["prepared"]["holds"]),
        "capped_at_2s": sum(1 for r in real if r["settle"]["capped_at_2s"]),
    }


def element(doc):
    e = doc.get("element") or {}
    return {k: e.get(k) for k in ("control_type", "class", "automation_id", "framework_id")}


def reduce_matrix(folder):
    out = []
    for label in ORDER:
        for session in ("1", "2"):
            path = os.path.join(folder, "%s-s%s.json" % (label, session))
            if not os.path.exists(path):
                continue
            doc = json.load(open(path))
            entry = {"toolkit": label, "session": int(session), "auto_set_focus": doc.get("auto_set_focus"), "element": element(doc), "cells": []}
            groups = collections.OrderedDict()
            for row in doc.get("rows", []):
                groups.setdefault((row["call"], row["arrangement"]), []).append(row)
            for (call, arrangement), rows in groups.items():
                entry["cells"].append(dict(call=call, arrangement=arrangement, **cell(rows)))
            out.append(entry)
    return out


def reduce_act(folder):
    out = []
    for path in sorted(glob.glob(os.path.join(folder, "*.json"))):
        name = os.path.basename(path)[:-5]
        doc = json.load(open(path))
        if doc.get("error"):
            out.append({"run": name, "not_run": doc["error"]})
            continue
        rows = doc["rows"]
        out.append({"run": name, "auto_set_focus": doc.get("auto_set_focus"), "element": element(doc), "trials": len(rows),
                    "ok": sum(1 for r in rows if r["outcome"]["ok"]),
                    "hresult": dict(collections.Counter(r["outcome"]["hresult"] or "none" for r in rows)),
                    "effect_changed": sum(1 for r in rows if r["detail"]["effect_changed"]),
                    "any_outside": sum(1 for r in rows if any_of(r, OUTSIDE)),
                    "foreground": sum(1 for r in rows if any_of(r, ["fg"])),
                    "winevents_foreground": sum(r["winevents"]["foreground"] for r in rows)})
    return out


def reduce_commands(folder):
    out = []
    for path in sorted(glob.glob(os.path.join(folder, "*-run*.json"))):
        if path.endswith(".args.json"):
            continue
        doc = json.load(open(path))
        row, env = doc["row"], doc["envelope"]
        data = env.get("data") or {}
        err = env.get("error") or {}
        out.append({"run": os.path.basename(path)[:-5], "command": doc["argv"][0], "headed": "--headed" in doc["argv"],
                    "version": env.get("version"), "ok": env.get("ok"), "error_code": err.get("code"),
                    "disposition": (data.get("disposition") or err.get("disposition")),
                    "steps": [{"label": s.get("label"), "outcome": s.get("outcome")} for s in data.get("steps", [])],
                    "percent_before": row["before"]["v_percent"], "percent_after": row["after"]["v_percent"],
                    "changed_at_any_time": row["changed_at_any_time"], "any_outside": any_of(row, OUTSIDE)})
    return out


def frac(c, key):
    return "%d/%d" % (c[key], c["trials"])


def matrix_table(default, off, out):
    index = {}
    for mode, entries in (("on", default), ("off", off)):
        for entry in entries:
            for c in entry["cells"]:
                index[(entry["toolkit"], c["call"], c["arrangement"], mode, entry["session"])] = c
    elements = {e["toolkit"]: e["element"] for e in default + off}
    out.append("| toolkit | call | arrangement | option | trials s1, s2 | HRESULT | scroll percent s1 | foreground s1, s2 | hwndFocus / UIA focus (s1+s2) | T thread active or focus | T above F / raised | IsIconic | WinEvents FG / FOCUS / MIN (s1+s2) |")
    out.append("|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for toolkit in ORDER:
        for call in CALLS:
            for arrangement in ("beside", "covered", "minimized"):
                for mode in ("on", "off"):
                    s = [index.get((toolkit, call, arrangement, mode, n)) for n in (1, 2)]
                    s = [x for x in s if x]
                    if not s:
                        continue
                    if all(x["trials"] == 0 for x in s):
                        out.append("| %s | %s | %s | %s | 0 | not run: %s | | | | | | | |" % (toolkit, SHORT[call], arrangement, mode, s[0]["skipped"]))
                        continue
                    live = [x for x in s if x["trials"]]
                    total = lambda key: sum(x[key] for x in live)
                    n = total("trials")
                    hres = collections.Counter()
                    for x in live:
                        hres.update(x["hresult"])
                    show = lambda v: "-" if v is None else "%.1f" % v
                    out.append("| %s | %s | %s | %s | %s | %s | %s to %s, %d of %d moved | %s | %d/%d / %d/%d | %d/%d | %d/%d / %d/%d | %d/%d | %d / %d / %d |" % (
                        toolkit, SHORT[call], arrangement, mode, ", ".join(str(x["trials"]) for x in s),
                        ", ".join("%s x%d" % kv for kv in hres.items()), show(live[0]["percent_first"]), show(live[0]["percent_last"]), total("moved"), n,
                        ", ".join(frac(x, "foreground") for x in live), total("focus_hwnd"), n, total("uia_focus"), n, total("t_thread"), n,
                        total("t_above_f"), n, total("raised"), n, total("iconic"), n,
                        sum(x["winevents"]["foreground"] for x in live), sum(x["winevents"]["object_focus"] for x in live), sum(x["winevents"]["minimize"] for x in live)))
    out.append("")
    out.append("Elements: " + "; ".join("%s = %s `%s` id `%s` (%s)" % (k, v["control_type"], v["class"], v["automation_id"], v["framework_id"]) for k, v in elements.items()))


def main(raw, captures, tables):
    os.makedirs(captures, exist_ok=True)
    default = reduce_matrix(os.path.join(raw, "out-a"))
    off = reduce_matrix(os.path.join(raw, "out-b"))
    a5 = reduce_matrix(os.path.join(raw, "out-a5"))
    act = reduce_act(os.path.join(raw, "out-act"))
    commands = reduce_commands(os.path.join(raw, "out-partb"))
    for name, value in (("scroll-default-client.json", default), ("scroll-autosetfocus-off.json", off),
                        ("scroll-default-client-with-scrollitem.json", a5), ("pattern-calls.json", act), ("shipped-commands.json", commands)):
        with open(os.path.join(captures, name), "w") as handle:
            json.dump(value, handle, indent=1, sort_keys=True)
            handle.write("\n")
    lines = ["## Part A: scroll calls, default client (option on) and AutoSetFocus off, two sessions", ""]
    matrix_table(default, off, lines)
    lines += ["", "## Part A: ScrollItemPattern.ScrollIntoView with the default client (one session)", ""]
    matrix_table([e for e in a5], [], lines)
    with open(tables, "w") as handle:
        handle.write("\n".join(lines) + "\n")
    print("captures:", ", ".join(sorted(os.listdir(captures))))


if __name__ == "__main__":
    main(*sys.argv[1:4])
