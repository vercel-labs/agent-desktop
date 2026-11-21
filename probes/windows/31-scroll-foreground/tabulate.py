#!/usr/bin/env python3
"""Writes the Part A markdown table: one row per toolkit, call, arrangement and session."""
import collections
import glob
import json
import sys

ORDER = ["win32", "winforms", "wpf", "directui", "xaml", "chromium", "chromium-div"]
CALLS = ["positive_control_SetFocus", "negative_control_CurrentName", "A1_Scroll", "A2_SetScrollPercent",
         "A5_ScrollIntoView", "A3_FindItemByProperty", "A4_Realize"]


def count(rows, fields, key="changed_at_any_time"):
    return sum(1 for r in rows if any(f in r[key] for f in fields))


def pct(rows):
    values = [(r["before"]["v_percent"], r["after"]["v_percent"]) for r in rows]
    show = lambda v: "-" if v is None else ("%.1f" % v)
    moved = sum(1 for b, a in values if b is not None and a is not None and abs(a - b) > 1e-9)
    return "%s to %s (%d moved)" % (show(values[0][0]), show(values[-1][1]), moved)


def main(folder, only_calls=None):
    docs = {}
    for path in glob.glob(folder + "/*-s*.json"):
        doc = json.load(open(path))
        docs[(doc.get("label"), str(doc.get("session")))] = doc
    print("| toolkit | call | arrangement | session | trials | HRESULT | scroll percent | foreground | focus (hwndFocus / UIA) | T thread active or focus | z-order (T above F / raised) | IsIconic | WinEvents FG / FOCUS / MIN |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for label in ORDER:
        for session in ("1", "2"):
            doc = docs.get((label, session))
            if not doc:
                continue
            if doc.get("no_vertically_scrollable_element") or doc.get("error"):
                print("| %s | all | all | %s | 0 | not run: %s | | | | | | | |" % (label, session, doc.get("error") or "no vertically scrollable element"))
                continue
            groups = collections.OrderedDict()
            for row in doc["rows"]:
                groups.setdefault((row["call"], row["arrangement"]), []).append(row)
            for call in CALLS:
                if only_calls and call not in only_calls:
                    continue
                for arrangement in ("beside", "covered", "minimized"):
                    rows = groups.get((call, arrangement))
                    if not rows:
                        continue
                    real = [r for r in rows if "skipped" not in r]
                    if not real:
                        print("| %s | %s | %s | %s | 0 | skipped: %s | | | | | | | |" % (label, call, arrangement, session, rows[0]["skipped"]))
                        continue
                    n = len(real)
                    hres = collections.Counter(r["outcome"]["hresult"] or ("crate sentinel %s" % r["outcome"]["crate_sentinel"]) for r in real)
                    hr = ", ".join("%s x%d" % kv for kv in hres.items())
                    ev = "%d / %d / %d" % (sum(r["winevents"]["foreground"] for r in real), sum(r["winevents"]["object_focus"] for r in real),
                                           sum(r["winevents"]["minimize_start"] + r["winevents"]["minimize_end"] for r in real))
                    note = "" if all(r["prepared"]["holds"] for r in real) else " (precondition failed %d)" % sum(1 for r in real if not r["prepared"]["holds"])
                    print("| %s | %s | %s | %s | %d%s | %s | %s | %d/%d | %d/%d / %d/%d | %d/%d | %d/%d / %d/%d | %d/%d | %s |" % (
                        label, call.replace("positive_control_", "control+ ").replace("negative_control_", "control- "), arrangement, session, n, note, hr, pct(real),
                        count(real, ["fg"]), n, count(real, ["fg_focus", "f_focus"]), n, count(real, ["uia_focus"]), n,
                        count(real, ["t_active", "t_focus"]), n, count(real, ["t_above_f"]), n, count(real, ["raised_past"]), n,
                        count(real, ["t_iconic"]), n, ev))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2].split(",") if len(sys.argv) > 2 else None)
