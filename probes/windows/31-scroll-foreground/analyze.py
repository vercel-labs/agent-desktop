#!/usr/bin/env python3
"""Summarizes the Part A captures: one line per target, session, arrangement and call."""
import collections
import glob
import json
import sys

OUTSIDE = ["fg", "fg_active", "fg_focus", "f_active", "f_focus", "uia_focus", "t_iconic", "t_visible", "t_above_f", "raised_past"]
T_LOCAL = ["t_active", "t_focus"]


def cell(rows):
    n = len(rows)
    ok = sum(1 for r in rows if r["outcome"]["ok"])
    hres = collections.Counter(r["outcome"]["hresult"] or ("sentinel %s" % r["outcome"]["crate_sentinel"]) for r in rows)
    outside = sum(1 for r in rows if any(f in r["changed_at_any_time"] for f in OUTSIDE))
    outside_end = sum(1 for r in rows if any(f in r["changed_at_end"] for f in OUTSIDE))
    local = sum(1 for r in rows if any(f in r["changed_at_any_time"] for f in T_LOCAL))
    moved = sum(1 for r in rows if "v_percent" in r["changed_at_any_time"])
    fields = collections.Counter(f for r in rows for f in r["changed_at_any_time"] if f != "v_percent")
    fg_ev = sum(r["winevents"]["foreground"] for r in rows)
    focus_ev = sum(r["winevents"]["object_focus"] for r in rows)
    min_ev = sum(r["winevents"]["minimize_start"] + r["winevents"]["minimize_end"] for r in rows)
    bad_prep = sum(1 for r in rows if not r["prepared"]["holds"])
    capped = sum(1 for r in rows if r["settle"]["capped_at_2s"])
    pct = ["%s>%s" % (fmt(r["before"]["v_percent"]), fmt(r["after"]["v_percent"])) for r in rows]
    ms = [r["outcome"]["ms"] for r in rows]
    return dict(n=n, ok=ok, hres=dict(hres), outside=outside, outside_end=outside_end, t_local=local, moved=moved,
                fields=dict(fields), fg_ev=fg_ev, focus_ev=focus_ev, min_ev=min_ev, bad_prep=bad_prep, capped=capped,
                pct=pct, ms_max=max(ms) if ms else 0)


def fmt(value):
    return "-" if value is None else ("%.1f" % value)


def main(folder):
    for path in sorted(glob.glob(folder + "/*-s*.json")):
        doc = json.load(open(path))
        label = "%s s%s" % (doc.get("label"), doc.get("session"))
        if doc.get("error") or doc.get("no_vertically_scrollable_element"):
            print(label, "NO DATA:", doc.get("error") or "no vertically scrollable element")
            continue
        element = doc["element"]
        print("== %s element=%s class=%s aid=%r fw=%s" % (label, element["control_type"], element["class"], element["automation_id"], element["framework_id"]))
        groups = collections.OrderedDict()
        for row in doc["rows"]:
            key = (row["arrangement"], row["call"])
            if "skipped" in row:
                groups.setdefault(key, []).append(row)
            else:
                groups.setdefault(key, []).append(row)
        for (arrangement, call), rows in groups.items():
            skipped = [r for r in rows if "skipped" in r]
            real = [r for r in rows if "skipped" not in r]
            if not real:
                print("   %-9s %-28s SKIPPED %s" % (arrangement, call, skipped[0]["skipped"]))
                continue
            c = cell(real)
            print("   %-9s %-28s n=%d ok=%d hr=%s outside=%d (end %d) t_local=%d moved=%d fgEv=%d focusEv=%d minEv=%d badprep=%d capped=%d fields=%s pct=%s maxms=%.0f%s" % (
                arrangement, call, c["n"], c["ok"], c["hres"], c["outside"], c["outside_end"], c["t_local"], c["moved"], c["fg_ev"],
                c["focus_ev"], c["min_ev"], c["bad_prep"], c["capped"], c["fields"], ",".join(c["pct"]), c["ms_max"],
                (" skipped=%d" % len(skipped)) if skipped else ""))


if __name__ == "__main__":
    main(sys.argv[1])
