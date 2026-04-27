#!/usr/bin/env python3
"""
Classify each Ultra recording's transcript as trivial or non-trivial via Gemini 2.5 Flash.

Goal: keep only transcripts that are *useful test data* for STT reverse-engineering —
i.e. non-trivial content where a regular STT would plausibly fail or struggle.

Outputs an incremental JSONL report. Does NOT delete anything.
Run delete_trivial.py afterwards to actually delete (with --apply).
"""
import argparse
import asyncio
import json
import os
import sys
from pathlib import Path

from google import genai
from google.genai import types

SCRIPT_DIR = Path(__file__).resolve().parent
RECORDINGS_DIR = SCRIPT_DIR / "playground" / "recordings"
REPORT_PATH = SCRIPT_DIR / "triviality_report.jsonl"

MODEL = "gemini-2.5-flash"
CONCURRENCY = 20

SYSTEM_PROMPT = """You judge whether a transcript represents a USE CASE that Whisper Large
struggles with.

Context: the user already runs Whisper Large locally (free) — it is already very accurate
on plain English and ordinary technical speech. We only want to keep transcripts that
exercise the EDGE CASES where Whisper-large is known to fail. Those are the only ones
worth sending to a stronger model for comparison.

Mark SUSPECT (keep — trivial=false) ONLY when the transcript shows at least one of these
hard-for-Whisper use cases. Quote the specific token in your reason:

1. Visible mistranscription artefacts in the existing text
   - garbled non-words (e.g. "RUGTREE", "re-identicate", "reckonition")
   - wrong-context homophones / phonetic guesses ("git rock trees" → likely "worktrees";
     "pacific" → "specific"; "metier" where a proper noun belongs)
   - mid-sentence cut-offs / truncation
   - repeated identical phrases over many seconds (silence-hallucination, e.g.
     "Thank you. Thank you. Thank you. ...")

2. Slang / casual / colloquial / informal speech
   - "bro", "dude", "gonna", "kinda", "wanna", "ain't", "yo"
   - swearing or emphatic intensifiers ("damn", "shit", "fuck", "WTF", "Jesus Christ")
   - heavy informal contractions, colloquial idioms
   - emotional / agitated tone words

3. Code-switching / mixed-language fragments (English with French / German / Spanish /
   Italian / etc. inline)

4. Spelled-out identifiers, file paths, URLs, exact code tokens, version strings, ports

5. Niche proper nouns: internal product names, indie tool names, niche repo names
   (Raycast, Tauri, BlackHole, NDI, MCP, gRPC, ACP, Easypanel, Dokploy, Iris, etc. —
   things that aren't household-name brands)

6. Numbers in technical context (ports, IDs, versions, percentages, model sizes)

7. Unusually short outputs (≤ 4 words) over a long audio duration — likely missed content

Mark CLEAN (delete — trivial=true) when the transcript is plain, fluent, well-formed English
even if it expresses a clear instruction, question, or multi-clause request. Plain English
("describe how this project works", "tell me more", "help me please", "I have a bunch of
things to restore, can you help me") is TRIVIAL. Whisper Large nails plain English.
Technical vocabulary that is well-formed and common (API, JSON, Git, Python, GitHub) is
NOT enough — we expect Whisper to know these.

Default when uncertain: CLEAN. Be strict — we expect roughly half the dataset to be CLEAN.

Return JSON: {"trivial": bool, "reason": "<= 14 words; if SUSPECT, quote the trigger token"}
where trivial=true means CLEAN (delete) and trivial=false means SUSPECT (keep).
"""


def load_existing_report() -> dict[str, dict]:
    if not REPORT_PATH.exists():
        return {}
    out = {}
    with REPORT_PATH.open() as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
                out[row["id"]] = row
            except (json.JSONDecodeError, KeyError):
                continue
    return out


def collect_transcripts() -> list[dict]:
    items = []
    for rid in sorted(os.listdir(RECORDINGS_DIR)):
        meta_path = RECORDINGS_DIR / rid / "meta.json"
        if not meta_path.exists():
            continue
        try:
            meta = json.loads(meta_path.read_text())
        except json.JSONDecodeError:
            continue
        text = (meta.get("result") or "").strip()
        if not text:
            continue
        items.append({
            "id": rid,
            "text": text,
            "duration_s": meta.get("duration", 0) / 1000,
        })
    return items


async def classify_one(client: genai.Client, sem: asyncio.Semaphore, item: dict) -> dict:
    async with sem:
        try:
            resp = await client.aio.models.generate_content(
                model=MODEL,
                contents=f"Transcript: {item['text']!r}",
                config=types.GenerateContentConfig(
                    system_instruction=SYSTEM_PROMPT,
                    temperature=0,
                    response_mime_type="application/json",
                    response_schema={
                        "type": "object",
                        "properties": {
                            "trivial": {"type": "boolean"},
                            "reason": {"type": "string"},
                        },
                        "required": ["trivial", "reason"],
                    },
                    thinking_config=types.ThinkingConfig(thinking_budget=0),
                ),
            )
            data = json.loads(resp.text)
            return {**item, "trivial": bool(data["trivial"]), "reason": data["reason"], "error": None}
        except Exception as e:
            return {**item, "trivial": None, "reason": None, "error": f"{type(e).__name__}: {e}"}


async def main_async(limit: int | None, force: bool) -> int:
    api_key = os.environ.get("GEMINI_API_KEY")
    if not api_key:
        print("ERROR: GEMINI_API_KEY not set", file=sys.stderr)
        return 1

    client = genai.Client(api_key=api_key)
    items = collect_transcripts()
    existing = {} if force else load_existing_report()

    todo = [it for it in items if it["id"] not in existing or existing[it["id"]].get("error")]
    skipped = len(items) - len(todo)
    if limit:
        todo = todo[:limit]

    print(f"Total transcripts: {len(items)} | already in report: {skipped} | to do this run: {len(todo)}")
    if not todo:
        return summarize(items, existing)

    sem = asyncio.Semaphore(CONCURRENCY)
    mode = "w" if force else "a"
    done = 0
    with REPORT_PATH.open(mode) as fh:
        if force:
            existing = {}
        tasks = [classify_one(client, sem, it) for it in todo]
        for coro in asyncio.as_completed(tasks):
            row = await coro
            fh.write(json.dumps(row) + "\n")
            fh.flush()
            existing[row["id"]] = row
            done += 1
            if done % 50 == 0 or done == len(todo):
                print(f"  {done}/{len(todo)} classified")

    return summarize(items, existing)


def summarize(items: list[dict], rows: dict[str, dict]) -> int:
    total = len(items)
    classified = [r for r in rows.values() if r.get("error") is None]
    errors = [r for r in rows.values() if r.get("error")]
    trivial = [r for r in classified if r["trivial"]]
    non_trivial = [r for r in classified if not r["trivial"]]

    trivial_seconds = sum(r["duration_s"] for r in trivial)
    non_trivial_seconds = sum(r["duration_s"] for r in non_trivial)

    def cost_v3(secs):
        billed_h = sum(max(d, 10) for d in [secs]) / 3600 if secs else 0
        return billed_h * 0.111

    print()
    print("=" * 60)
    print(f"Total: {total}  |  classified: {len(classified)}  |  errors: {len(errors)}")
    print(f"  TRIVIAL:     {len(trivial):>5}  ({trivial_seconds/60:6.1f} min)")
    print(f"  NON-TRIVIAL: {len(non_trivial):>5}  ({non_trivial_seconds/60:6.1f} min)")
    print()
    print(f"Re-transcribe NON-TRIVIAL only on Whisper Large V3:")
    nt_hours = non_trivial_seconds / 3600
    t_hours = trivial_seconds / 3600
    print(f"  cost: ${nt_hours * 0.111:.3f} (vs ${(nt_hours + t_hours) * 0.111:.3f} for everything)")
    print(f"  saved by skipping trivial: ${t_hours * 0.111:.3f}")
    print()
    print(f"Report: {REPORT_PATH}")
    if errors:
        print(f"  {len(errors)} errors — re-run to retry.")
    return 0


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--limit", type=int, help="Only classify N transcripts (for testing)")
    p.add_argument("--force", action="store_true", help="Re-classify everything from scratch")
    args = p.parse_args()
    sys.exit(asyncio.run(main_async(args.limit, args.force)))


if __name__ == "__main__":
    main()
