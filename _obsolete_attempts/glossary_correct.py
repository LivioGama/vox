#!/usr/bin/env python3
"""
Surgically correct mistranscriptions in Groq transcripts using a curated glossary.

Approach: the curated glossary (glossary.json) is the ONLY source of truth for what
the LLM is allowed to change. The LLM is instructed to leave EVERYTHING else verbatim
(no punctuation tweaks, no casing fixes, no grammar polish, no reordering, no inventing
content). It can only swap a token that is unambiguously a phonetic mistranscription of
a glossary term for the canonical glossary term.

Inputs:
  /Users/livio/Documents/vox/glossary.json
  /Users/livio/Documents/vox/playground/groq_transcripts/<id>.json

Outputs:
  /Users/livio/Documents/vox/playground/groq_improved/<id>.txt
  /Users/livio/Documents/vox/glossary_corrections.jsonl   (audit log)
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
GLOSSARY_PATH = SCRIPT_DIR / "glossary.json"
GROQ_DIR = SCRIPT_DIR / "playground" / "groq_transcripts"
IMPROVED_DIR = SCRIPT_DIR / "playground" / "groq_improved"
AUDIT_PATH = SCRIPT_DIR / "glossary_corrections.jsonl"

MODEL = "gemini-2.5-flash"
CONCURRENCY = 20


def build_glossary_block(glossary: list[dict]) -> str:
    """Render the glossary as a stable, human-readable reference list."""
    lines = ["GLOSSARY (canonical term : known mistranscriptions):"]
    for entry in glossary:
        term = entry["term"]
        miss = entry.get("mistranscriptions") or []
        if miss:
            lines.append(f"- {term} : {', '.join(miss)}")
        else:
            lines.append(f"- {term} : (no known mistranscriptions; recognise only by clear phonetic match)")
    return "\n".join(lines)


def build_system_prompt(glossary: list[dict]) -> str:
    glossary_block = build_glossary_block(glossary)
    return f"""You are a surgical transcript corrector. Your ONLY job is to fix tokens in
a speech-to-text transcript that are obvious phonetic mistranscriptions of a term in the
GLOSSARY below. You must NOT do anything else.

{glossary_block}

STRICT RULES — violating any of these is a failure:

1. Return the transcript VERBATIM except for glossary substitutions.
   - Do not change punctuation, capitalisation, spacing, line breaks, or word order.
   - Do not add, remove, or reorder any words.
   - Do not "polish" grammar, fluency, articles, or filler words.
   - Do not translate. If the transcript is in French (or any language), keep it in that
     language and only swap glossary tokens.
   - Do not invent content the speaker did not say.
   - Do not "fix" a word just because it looks unusual — only fix it if it is clearly a
     mistranscription of a specific glossary term listed above.

2. Only replace a token when ALL of these hold:
   - The surrounding context makes it unambiguous that the speaker meant the glossary term.
   - The token is either listed verbatim under that term's mistranscriptions, OR is a
     near-identical phonetic spelling of the canonical term (e.g. "dock ploy" -> "Dokploy",
     "Cloud Code" -> "Claude Code" when clearly referring to the CLI).
   - You are confident — if in doubt, leave it alone.

3. Common-English-word traps — DO NOT replace these unless the technical context is
   absolutely unambiguous:
   - "cloud" -> "Claude" only when the speaker is clearly talking about the Claude AI
     model / Claude Code / Anthropic's product. "the cloud" in an infrastructure sentence
     stays "the cloud".
   - "deploy" -> "Dokploy" only when clearly the product name, never the verb "to deploy".
   - "qualify" -> "Coolify" only when clearly the product name.
   - "much" -> "cmux" only when clearly the tool name.
   - "burn" -> "Bun" only when clearly the JavaScript runtime.
   - "Grok" -> "Groq" only when clearly the inference provider, not the xAI chatbot.
   - "Ralph" -> "RALF" only when context clearly refers to the acronym, not Ralph Wiggum.
   - "UI" -> "OpenUI" only when clearly the product, not generic UI.
   - "VPS" -> "TPS" only when clearly tokens-per-second, not virtual private server.
   - "deploy" by itself in plain infra speech: leave it. Never change a verb.

4. Token boundaries: when a glossary mistranscription spans multiple words (e.g.
   "dock ploy"), replace the whole span with the canonical term ("Dokploy"). When it is
   one word, replace one word. Match length sensibly.

5. If the transcript contains zero glossary mistranscriptions, return it COMPLETELY
   UNCHANGED and an empty fixes array. This is the expected outcome for most transcripts.

6. Output format (STRICT JSON, matches the schema):
   - "corrected": the full transcript text after surgical edits (verbatim if none).
   - "fixes": array of {{"wrong": "<exact substring you replaced>", "correct": "<canonical glossary term>"}}.
     One entry per replacement. Empty array if nothing changed.

You will receive one transcript per request, labelled `TRANSCRIPT TO CORRECT:`. Process
only that transcript and return only the JSON object.
"""


RESPONSE_SCHEMA = {
    "type": "object",
    "properties": {
        "corrected": {"type": "string"},
        "fixes": {
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "wrong": {"type": "string"},
                    "correct": {"type": "string"},
                },
                "required": ["wrong", "correct"],
            },
        },
    },
    "required": ["corrected", "fixes"],
}


def load_glossary() -> list[dict]:
    return json.loads(GLOSSARY_PATH.read_text())


def collect_inputs() -> list[dict]:
    items = []
    for fp in sorted(GROQ_DIR.glob("*.json")):
        rid = fp.stem
        try:
            data = json.loads(fp.read_text())
        except json.JSONDecodeError:
            continue
        text = (data.get("text") or "").strip()
        if not text:
            continue
        items.append({"id": rid, "text": text})
    return items


async def correct_one(
    client: genai.Client,
    sem: asyncio.Semaphore,
    system_prompt: str,
    item: dict,
) -> dict:
    async with sem:
        try:
            resp = await client.aio.models.generate_content(
                model=MODEL,
                contents=f"TRANSCRIPT TO CORRECT:\n{item['text']}",
                config=types.GenerateContentConfig(
                    system_instruction=system_prompt,
                    temperature=0,
                    response_mime_type="application/json",
                    response_schema=RESPONSE_SCHEMA,
                    thinking_config=types.ThinkingConfig(thinking_budget=0),
                ),
            )
            data = json.loads(resp.text)
            corrected = data.get("corrected", "").strip()
            fixes = data.get("fixes") or []
            if not corrected:
                # Defensive: never overwrite with empty
                corrected = item["text"]
            return {
                "id": item["id"],
                "original": item["text"],
                "corrected": corrected,
                "fixes": fixes,
                "error": None,
            }
        except Exception as e:
            return {
                "id": item["id"],
                "original": item["text"],
                "corrected": None,
                "fixes": [],
                "error": f"{type(e).__name__}: {e}",
            }


async def main_async(limit: int | None, force: bool) -> int:
    api_key = os.environ.get("GEMINI_API_KEY")
    if not api_key:
        print("ERROR: GEMINI_API_KEY not set", file=sys.stderr)
        return 1

    IMPROVED_DIR.mkdir(parents=True, exist_ok=True)

    glossary = load_glossary()
    system_prompt = build_system_prompt(glossary)
    print(f"Loaded glossary: {len(glossary)} entries")
    print(f"System prompt size: {len(system_prompt)} chars")

    client = genai.Client(api_key=api_key)
    items = collect_inputs()
    print(f"Total Groq transcripts: {len(items)}")

    if not force:
        todo = [it for it in items if not (IMPROVED_DIR / f"{it['id']}.txt").exists()]
    else:
        todo = list(items)
    skipped = len(items) - len(todo)
    if limit:
        todo = todo[:limit]
    print(f"Already done: {skipped} | to process this run: {len(todo)}")
    if not todo:
        print("Nothing to do.")
        return 0

    sem = asyncio.Semaphore(CONCURRENCY)
    audit_mode = "w" if force else "a"

    done = 0
    n_with_fixes = 0
    n_errors = 0
    total_fixes = 0
    fix_counts: dict[str, int] = {}

    with AUDIT_PATH.open(audit_mode) as audit_fh:
        tasks = [correct_one(client, sem, system_prompt, it) for it in todo]
        for coro in asyncio.as_completed(tasks):
            row = await coro
            if row["error"]:
                n_errors += 1
                audit_fh.write(json.dumps(row) + "\n")
                audit_fh.flush()
                done += 1
                if done % 25 == 0 or done == len(todo):
                    print(f"  {done}/{len(todo)} processed (errors: {n_errors})")
                continue

            # Write the corrected text out
            (IMPROVED_DIR / f"{row['id']}.txt").write_text(row["corrected"])

            if row["fixes"]:
                n_with_fixes += 1
                total_fixes += len(row["fixes"])
                for fx in row["fixes"]:
                    canon = fx.get("correct", "?")
                    fix_counts[canon] = fix_counts.get(canon, 0) + 1

            audit_fh.write(json.dumps(row) + "\n")
            audit_fh.flush()
            done += 1
            if done % 25 == 0 or done == len(todo):
                print(f"  {done}/{len(todo)} processed (with fixes: {n_with_fixes}, errors: {n_errors})")

    print()
    print("=" * 60)
    print(f"Processed:        {done}")
    print(f"With >=1 fix:     {n_with_fixes}")
    print(f"Total fixes:      {total_fixes}")
    print(f"Errors:           {n_errors}")
    if fix_counts:
        print()
        print("Fixes per glossary term (top 20):")
        for term, c in sorted(fix_counts.items(), key=lambda x: -x[1])[:20]:
            print(f"  {c:>4}  {term}")
    print()
    print(f"Outputs: {IMPROVED_DIR}")
    print(f"Audit:   {AUDIT_PATH}")
    return 0


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--limit", type=int, help="Process only N transcripts (for testing)")
    p.add_argument("--force", action="store_true", help="Re-process even if output exists")
    args = p.parse_args()
    sys.exit(asyncio.run(main_async(args.limit, args.force)))


if __name__ == "__main__":
    main()
