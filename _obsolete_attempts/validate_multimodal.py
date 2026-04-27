#!/usr/bin/env python3
"""
Validation gate: send (audio + Groq draft + glossary) to Gemini 2.5 Flash
multimodal for 10 hand-picked failure cases. Compare audio-grounded output
vs Groq vs Meta.
"""
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
GLOSSARY_PATH = SCRIPT_DIR / "glossary.json"
IDS_PATH = SCRIPT_DIR / "validation_ids.json"
OUT_PATH = SCRIPT_DIR / "validation_results.json"

MODEL = "gemini-2.5-flash"


def build_glossary_hint() -> str:
    g = json.loads(GLOSSARY_PATH.read_text())
    terms = [e["term"] for e in g[:50]]
    return ", ".join(terms)


SYSTEM_PROMPT = """You are an expert speech-to-text transcriber. Listen carefully to the
audio and produce the most accurate transcript possible.

You also receive a draft transcript from Groq Whisper (which often makes errors on niche
technical terms) and a glossary of niche terms commonly mentioned by this speaker.

Use the audio as ground truth. Use the draft only as a starting point. Use the glossary
to correctly spell technical terms (e.g., if you hear something close to "dock ploy",
prefer "Dokploy"; if you hear "cloud code" but it sounds like "Claude Code" given the
context, prefer "Claude Code").

Output a JSON object with:
- transcript: your best transcript of what was actually said
- changes: list of {from, to, reason} for each correction you made vs the Groq draft
- confidence: "high" | "medium" | "low"

Be faithful to what was actually said. Don't invent content. Don't translate."""


async def transcribe_one(client, sem, rid: str, glossary_hint: str) -> dict:
    async with sem:
        audio_path = RECORDINGS_DIR / rid / "output.wav"
        groq_path = GROQ_DIR / f"{rid}.json"
        meta_path = RECORDINGS_DIR / rid / "meta.json"

        try:
            audio_bytes = audio_path.read_bytes()
            groq_text = (json.loads(groq_path.read_text()).get("text") or "").strip()
            meta_text = (json.loads(meta_path.read_text()).get("result") or "").strip()
        except Exception as e:
            return {"id": rid, "error": f"load: {e}"}

        try:
            resp = await client.aio.models.generate_content(
                model=MODEL,
                contents=[
                    f"GLOSSARY (this speaker's niche terms): {glossary_hint}\n\n"
                    f"GROQ DRAFT (often wrong on niche terms): {groq_text!r}\n\n"
                    f"Listen to the attached audio and produce the corrected transcript.",
                    types.Part.from_bytes(data=audio_bytes, mime_type="audio/wav"),
                ],
                config=types.GenerateContentConfig(
                    system_instruction=SYSTEM_PROMPT,
                    temperature=0,
                    response_mime_type="application/json",
                    response_schema={
                        "type": "object",
                        "properties": {
                            "transcript": {"type": "string"},
                            "changes": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "from": {"type": "string"},
                                        "to": {"type": "string"},
                                        "reason": {"type": "string"},
                                    },
                                },
                            },
                            "confidence": {"type": "string"},
                        },
                        "required": ["transcript", "confidence"],
                    },
                    thinking_config=types.ThinkingConfig(thinking_budget=0),
                ),
            )
            data = json.loads(resp.text)
            return {
                "id": rid,
                "groq": groq_text,
                "meta": meta_text,
                "gemini": data.get("transcript", "").strip(),
                "changes": data.get("changes", []),
                "confidence": data.get("confidence", ""),
                "audio_kb": len(audio_bytes) / 1024,
            }
        except Exception as e:
            return {"id": rid, "error": f"gemini: {type(e).__name__}: {e}"}


async def main():
    api_key = os.environ.get("GEMINI_API_KEY")
    if not api_key:
        sys.exit("GEMINI_API_KEY not set")

    ids = json.loads(IDS_PATH.read_text())
    glossary_hint = build_glossary_hint()

    print(f"Validating {len(ids)} audios with {MODEL} (multimodal)...")
    print(f"Glossary terms in prompt: {len(glossary_hint.split(','))}\n")

    client = genai.Client(api_key=api_key)
    sem = asyncio.Semaphore(5)

    tasks = [transcribe_one(client, sem, rid, glossary_hint) for rid in ids]
    results = await asyncio.gather(*tasks)

    OUT_PATH.write_text(json.dumps(results, indent=2, ensure_ascii=False))

    print("=" * 80)
    for r in results:
        if "error" in r:
            print(f"\n[{r['id']}] ERROR: {r['error']}")
            continue
        print(f"\n[{r['id']}]  audio={r['audio_kb']:.0f}KB  conf={r['confidence']}")
        print(f"  GROQ:   {r['groq'][:200]}")
        print(f"  META:   {r['meta'][:200]}")
        print(f"  GEMINI: {r['gemini'][:200]}")
        if r.get("changes"):
            for c in r["changes"][:3]:
                print(f"    fix: {c.get('from','')!r} → {c.get('to','')!r} ({c.get('reason','')})")
    print()
    print(f"Saved → {OUT_PATH}")


if __name__ == "__main__":
    asyncio.run(main())
