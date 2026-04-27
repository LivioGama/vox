#!/usr/bin/env python3
"""
Batch process all Groq transcripts to improve them using learned patterns.
Reads:  playground/groq_transcripts/<id>.json
Writes: playground/groq_improved/<id>.txt
"""
import asyncio
import json
import os
from pathlib import Path

from improve_transcript import load_error_patterns, improve_transcript

SCRIPT_DIR = Path(__file__).resolve().parent
GROQ_DIR = SCRIPT_DIR / "playground" / "groq_transcripts"
IMPROVED_DIR = SCRIPT_DIR / "playground" / "groq_improved"


async def process_all_groq_transcripts():
    patterns = load_error_patterns()
    IMPROVED_DIR.mkdir(parents=True, exist_ok=True)

    todo = []
    for groq_file in sorted(GROQ_DIR.glob("*.json")):
        recording_id = groq_file.stem
        out_file = IMPROVED_DIR / f"{recording_id}.txt"
        if out_file.exists():
            continue
        try:
            data = json.loads(groq_file.read_text())
            text = (data.get("text") or "").strip()
        except json.JSONDecodeError:
            continue
        if text:
            todo.append((recording_id, text, out_file))

    print(f"Found {len(todo)} Groq transcripts to improve")
    if not todo:
        return

    sem = asyncio.Semaphore(10)

    async def process_one(recording_id, text, out_file):
        async with sem:
            try:
                improved = await improve_transcript(text, patterns)
                out_file.write_text(improved)
                return recording_id, len(text.split()), len(improved.split()), True
            except Exception as e:
                print(f"Error processing {recording_id}: {e}")
                return recording_id, 0, 0, False

    tasks = [process_one(rid, text, out) for rid, text, out in todo]

    done = 0
    successful = 0
    total_orig = 0
    total_improved = 0

    for coro in asyncio.as_completed(tasks):
        rid, orig_words, imp_words, ok = await coro
        done += 1
        if ok:
            successful += 1
            total_orig += orig_words
            total_improved += imp_words

        if done % 50 == 0 or done == len(tasks):
            print(f"  {done}/{len(tasks)} processed ({successful} ok)")

    print(f"\nProcessed {successful}/{len(todo)} transcripts")
    if successful:
        print(f"Original words:  {total_orig}")
        print(f"Improved words:  {total_improved}")
        print(f"Avg word change: {(total_improved - total_orig) / successful:+.2f} per transcript")
    print(f"Improved transcripts: {IMPROVED_DIR}")


if __name__ == "__main__":
    asyncio.run(process_all_groq_transcripts())
