#!/usr/bin/env python3
"""
Compare transcripts before and after improvement to measure quality gains.
Reads:
  playground/recordings/<id>/meta.json   (ground truth)
  playground/groq_transcripts/<id>.json  (original Groq)
  playground/groq_improved/<id>.txt      (improved by LLM)
"""
import json
import os
from pathlib import Path
from typing import List, Tuple

SCRIPT_DIR = Path(__file__).resolve().parent
RECORDINGS_DIR = SCRIPT_DIR / "playground" / "recordings"
GROQ_DIR = SCRIPT_DIR / "playground" / "groq_transcripts"
IMPROVED_DIR = SCRIPT_DIR / "playground" / "groq_improved"


def word_error_rate(reference: str, hypothesis: str) -> float:
    """Levenshtein-based WER."""
    ref = reference.lower().split()
    hyp = hypothesis.lower().split()
    if not ref:
        return 0.0 if not hyp else 1.0

    prev = list(range(len(hyp) + 1))
    for i, r in enumerate(ref, 1):
        curr = [i]
        for j, h in enumerate(hyp, 1):
            curr.append(min(
                prev[j] + 1,           # deletion
                curr[j - 1] + 1,       # insertion
                prev[j - 1] + (r != h) # substitution
            ))
        prev = curr
    return prev[-1] / len(ref)


def load_comparisons() -> List[Tuple[str, str, str, str]]:
    rows = []
    for improved_file in IMPROVED_DIR.glob("*.txt"):
        rid = improved_file.stem
        meta_file = RECORDINGS_DIR / rid / "meta.json"
        groq_file = GROQ_DIR / f"{rid}.json"
        if not (meta_file.exists() and groq_file.exists()):
            continue
        try:
            meta = (json.loads(meta_file.read_text()).get("result") or "").strip()
            groq_orig = (json.loads(groq_file.read_text()).get("text") or "").strip()
            groq_imp = improved_file.read_text().strip()
        except json.JSONDecodeError:
            continue
        if meta and groq_orig and groq_imp:
            rows.append((rid, meta, groq_orig, groq_imp))
    return rows


def main():
    rows = load_comparisons()
    if not rows:
        print("No improved transcripts found. Run batch_improve_transcripts.py first.")
        return

    print(f"Analyzing {len(rows)} transcript improvements...\n")

    orig_wers, imp_wers = [], []
    improvements, degradations = [], []

    for rid, meta, groq_orig, groq_imp in rows:
        orig = word_error_rate(meta, groq_orig)
        imp = word_error_rate(meta, groq_imp)
        orig_wers.append(orig)
        imp_wers.append(imp)
        delta = orig - imp  # positive = better

        if delta > 0.05:
            improvements.append((rid, orig, imp, delta, meta, groq_orig, groq_imp))
        elif delta < -0.05:
            degradations.append((rid, orig, imp, delta, meta, groq_orig, groq_imp))

    avg_orig = sum(orig_wers) / len(orig_wers)
    avg_imp = sum(imp_wers) / len(imp_wers)
    avg_delta = avg_orig - avg_imp
    pct = (avg_delta / avg_orig * 100) if avg_orig else 0.0

    print("=" * 80)
    print("OVERALL RESULTS")
    print("=" * 80)
    print(f"Transcripts analyzed:    {len(rows)}")
    print(f"Average WER (original):  {avg_orig:.3f}")
    print(f"Average WER (improved):  {avg_imp:.3f}")
    print(f"Average improvement:     {avg_delta:+.3f} ({pct:+.1f}%)")
    print()
    print(f"Significantly improved:  {len(improvements)} ({len(improvements)/len(rows)*100:.1f}%)")
    print(f"Significantly degraded:  {len(degradations)} ({len(degradations)/len(rows)*100:.1f}%)")
    print(f"No significant change:   {len(rows) - len(improvements) - len(degradations)}")

    if improvements:
        print("\n" + "=" * 80)
        print("TOP 10 IMPROVEMENTS")
        print("=" * 80)
        for i, (rid, o, n, d, meta, go, gi) in enumerate(sorted(improvements, key=lambda x: -x[3])[:10], 1):
            print(f"\n{i}. {rid}  WER {o:.3f} → {n:.3f}  (Δ{d:+.3f})")
            print(f"   META: {meta[:120]}")
            print(f"   ORIG: {go[:120]}")
            print(f"   IMPR: {gi[:120]}")

    if degradations:
        print("\n" + "=" * 80)
        print("TOP 5 DEGRADATIONS (review)")
        print("=" * 80)
        for i, (rid, o, n, d, meta, go, gi) in enumerate(sorted(degradations, key=lambda x: x[3])[:5], 1):
            print(f"\n{i}. {rid}  WER {o:.3f} → {n:.3f}  (Δ{d:+.3f})")
            print(f"   META: {meta[:120]}")
            print(f"   ORIG: {go[:120]}")
            print(f"   IMPR: {gi[:120]}")


if __name__ == "__main__":
    main()
