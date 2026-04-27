#!/usr/bin/env python3
"""
Extract a clean glossary of niche terms that Groq Whisper mistranscribes.

Stage 1 (Flash):  Per (Meta, Groq) pair, identify niche-term mistranscriptions.
Stage 2 (Pro):    Consolidate, dedupe, validate against world knowledge.

Skips pairs that look like translations / different-language outputs.
Outputs glossary.json with {term, mistranscriptions, category, frequency}.
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
GROQ_DIR = SCRIPT_DIR / "playground" / "groq_transcripts"
RAW_CANDIDATES = SCRIPT_DIR / "glossary_candidates.jsonl"
GLOSSARY_PATH = SCRIPT_DIR / "glossary.json"

STAGE1_MODEL = "gemini-2.5-flash"
STAGE2_MODEL = "gemini-2.5-pro"
CONCURRENCY = 20

CURATOR_PROMPT = """You compare two STT transcripts of the same audio. Identify niche
domain-specific tokens (product names, library names, frameworks, code identifiers, jargon,
acronyms) where the two versions disagree and one is clearly more plausible.

EXTRACT only when:
- One version contains a real-world technical term, product, library, tool, framework,
  or proper noun (e.g. Dokploy, Raycast, Qdrant, Kubernetes, MCP, ACP, Iris, BlackHole)
- The other version has a phonetic mistranscription of it (e.g. "dock ploy", "Skudrant",
  "c-max" for "cmux", "DOG PLY", "Cuban Edis" for "Kubernetes")

IGNORE entirely — return empty list — when:
- The pair is in different languages (one English, one French/Korean/etc.) — these are
  not transcription errors, they are language-detection differences
- Disagreements involve only common English words (the/this/that, a/an, is/this)
- Differences are punctuation, casing, contractions, fillers, or sentence structure
- Both versions look plausibly correct
- You can't tell which version is right
- Common technical terms both Whisper variants would know (API, JSON, HTTP, Python, Git)

Bias to empty. A noisy glossary is worse than an empty one.

Return JSON list: [{"correct": str, "wrong": str, "category": "tool|library|product|acronym|name|other", "confidence": "low|medium|high"}]
"""

CONSOLIDATOR_PROMPT = """You receive a flat list of glossary candidates extracted from STT
comparison. Multiple raw entries may refer to the same canonical term.

Your job: produce the final clean glossary using YOUR world knowledge.

For each real canonical term:
- Pick the single best canonical spelling (use the actual product/library casing)
- Group ALL phonetic mistranscriptions of it under it (dedupe near-duplicates)
- Compute frequency = number of raw entries that mapped to this term

Drop entries that:
- Are not real-world terms you can verify (LLM hallucinations from stage 1)
- Are common English words that don't need biasing
- Have low confidence and frequency 1
- Are personal/internal names you can't verify (unless frequency ≥ 3)

WORLD-KNOWLEDGE OVERRIDE: if a candidate marks "Skudrant" as correct and "Qdrant" as wrong,
SWAP THEM — you know Qdrant is real, Skudrant is not. Same for any case where the LLM had
the direction backward.

Return JSON list, sorted by frequency descending:
[{"term": str, "mistranscriptions": [str, ...], "category": str, "frequency": int}]

Be strict. Exclude anything you wouldn't bet money is real."""


def has_non_latin(text: str) -> bool:
    """Skip pairs with substantial non-Latin chars (Korean, Japanese, Chinese, Cyrillic, etc.)."""
    non_latin = sum(
        1
        for c in text
        if c.isalpha() and ord(c) > 0x017F  # beyond Latin Extended-A
    )
    return non_latin > 3


def load_pairs() -> list[tuple[str, str, str]]:
    pairs = []
    skipped_lang = 0
    skipped_same = 0
    for groq_file in sorted(GROQ_DIR.glob("*.json")):
        rid = groq_file.stem
        meta_file = RECORDINGS_DIR / rid / "meta.json"
        if not meta_file.exists():
            continue
        try:
            meta = (json.loads(meta_file.read_text()).get("result") or "").strip()
            groq = (json.loads(groq_file.read_text()).get("text") or "").strip()
        except json.JSONDecodeError:
            continue
        if not meta or not groq:
            continue
        if meta == groq:
            skipped_same += 1
            continue
        if has_non_latin(meta) or has_non_latin(groq):
            skipped_lang += 1
            continue
        pairs.append((rid, meta, groq))
    print(f"Loaded {len(pairs)} pairs (skipped {skipped_same} identical, {skipped_lang} non-Latin)")
    return pairs


async def extract_one(client, sem, rid, meta, groq):
    async with sem:
        try:
            resp = await client.aio.models.generate_content(
                model=STAGE1_MODEL,
                contents=f"META:  {meta!r}\nGROQ:  {groq!r}",
                config=types.GenerateContentConfig(
                    system_instruction=CURATOR_PROMPT,
                    temperature=0,
                    response_mime_type="application/json",
                    response_schema={
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "correct": {"type": "string"},
                                "wrong": {"type": "string"},
                                "category": {"type": "string"},
                                "confidence": {"type": "string"},
                            },
                            "required": ["correct", "wrong", "category", "confidence"],
                        },
                    },
                    thinking_config=types.ThinkingConfig(thinking_budget=0),
                ),
            )
            return rid, json.loads(resp.text), None
        except Exception as e:
            return rid, None, f"{type(e).__name__}: {e}"


async def stage1_extract(pairs, force):
    if RAW_CANDIDATES.exists() and not force:
        n = sum(1 for _ in RAW_CANDIDATES.open())
        print(f"Stage 1: using existing {RAW_CANDIDATES.name} ({n} entries) — pass --force to redo")
        return

    api_key = os.environ.get("GEMINI_API_KEY")
    if not api_key:
        sys.exit("ERROR: GEMINI_API_KEY not set")

    client = genai.Client(api_key=api_key)
    sem = asyncio.Semaphore(CONCURRENCY)

    print(f"Stage 1: extracting candidates from {len(pairs)} pairs (Flash)...")

    total = 0
    errors = 0
    with RAW_CANDIDATES.open("w") as fh:
        tasks = [extract_one(client, sem, rid, m, g) for rid, m, g in pairs]
        done = 0
        for coro in asyncio.as_completed(tasks):
            rid, cands, err = await coro
            done += 1
            if err:
                errors += 1
            elif cands:
                for c in cands:
                    fh.write(json.dumps({"recording_id": rid, **c}) + "\n")
                    total += 1
            if done % 50 == 0 or done == len(tasks):
                print(f"  {done}/{len(tasks)} pairs, {total} candidates, {errors} errors")

    print(f"Stage 1 done: {total} raw candidates → {RAW_CANDIDATES.name}")


async def stage2_consolidate():
    if not RAW_CANDIDATES.exists():
        sys.exit(f"Run stage 1 first — {RAW_CANDIDATES} missing")

    candidates = [json.loads(l) for l in RAW_CANDIDATES.open() if l.strip()]
    if not candidates:
        print("No candidates from stage 1.")
        GLOSSARY_PATH.write_text("[]")
        return

    print(f"Stage 2: consolidating {len(candidates)} candidates with {STAGE2_MODEL}...")

    api_key = os.environ.get("GEMINI_API_KEY")
    client = genai.Client(api_key=api_key)

    payload = [
        {k: c.get(k, "") for k in ("correct", "wrong", "category", "confidence")}
        for c in candidates
    ]

    resp = await client.aio.models.generate_content(
        model=STAGE2_MODEL,
        contents=json.dumps(payload),
        config=types.GenerateContentConfig(
            system_instruction=CONSOLIDATOR_PROMPT,
            temperature=0,
            response_mime_type="application/json",
            response_schema={
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "term": {"type": "string"},
                        "mistranscriptions": {"type": "array", "items": {"type": "string"}},
                        "category": {"type": "string"},
                        "frequency": {"type": "integer"},
                    },
                    "required": ["term", "mistranscriptions", "category", "frequency"],
                },
            },
        ),
    )

    glossary = json.loads(resp.text)
    GLOSSARY_PATH.write_text(json.dumps(glossary, indent=2, ensure_ascii=False))

    print(f"Stage 2 done: {len(glossary)} terms → {GLOSSARY_PATH.name}")
    print()
    print("=" * 70)
    print("GLOSSARY PREVIEW")
    print("=" * 70)
    for e in glossary[:30]:
        miss = ", ".join(e["mistranscriptions"][:4])
        print(f"  {e['term']:<22} {e['frequency']:>3}x  [{e['category']}]  ← {miss}")
    if len(glossary) > 30:
        print(f"  ... and {len(glossary) - 30} more")


async def main_async(args):
    pairs = load_pairs()
    if args.limit:
        pairs = pairs[: args.limit]
        print(f"Limited to {len(pairs)} for testing")
    await stage1_extract(pairs, args.force)
    await stage2_consolidate()


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--limit", type=int, help="Limit pairs (for validation runs)")
    p.add_argument("--force", action="store_true", help="Redo stage 1 from scratch")
    args = p.parse_args()
    asyncio.run(main_async(args))


if __name__ == "__main__":
    main()
