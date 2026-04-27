#!/usr/bin/env python3
"""
Delete recordings that classify_triviality.py marked as trivial.

Dry-run by default. Pass --apply to actually delete.
"""
import argparse
import json
import shutil
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
RECORDINGS_DIR = SCRIPT_DIR / "playground" / "recordings"
REPORT_PATH = SCRIPT_DIR / "triviality_report.jsonl"


def load_trivial_ids() -> list[dict]:
    if not REPORT_PATH.exists():
        sys.exit(f"ERROR: {REPORT_PATH} not found. Run classify_triviality.py first.")
    rows = []
    with REPORT_PATH.open() as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            r = json.loads(line)
            if r.get("trivial") is True and not r.get("error"):
                rows.append(r)
    return rows


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--apply", action="store_true", help="Actually delete (default: dry-run)")
    args = p.parse_args()

    trivial = load_trivial_ids()
    print(f"{len(trivial)} recordings marked trivial.")
    print()

    missing = []
    to_delete = []
    for r in trivial:
        path = RECORDINGS_DIR / r["id"]
        if not path.exists():
            missing.append(r["id"])
            continue
        to_delete.append((path, r))

    print(f"  on disk:  {len(to_delete)}")
    print(f"  missing:  {len(missing)}")
    print()

    if not args.apply:
        print("DRY RUN — pass --apply to actually delete. Sample:")
        for path, r in to_delete[:10]:
            print(f"  rm -rf {path.name}  ({r['duration_s']:.1f}s) {r['text'][:60]!r}")
        if len(to_delete) > 10:
            print(f"  ... and {len(to_delete) - 10} more")
        return

    for path, _ in to_delete:
        shutil.rmtree(path)
    print(f"Deleted {len(to_delete)} recording directories.")


if __name__ == "__main__":
    main()
