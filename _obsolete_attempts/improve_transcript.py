#!/usr/bin/env python3
"""
Improve Groq transcripts using learned error patterns from Meta vs Groq analysis.
Uses few-shot learning with dynamically selected examples.
"""
import argparse
import json
import os
import sys
from pathlib import Path
from typing import Dict, List, Tuple

from google import genai
from google.genai import types

SCRIPT_DIR = Path(__file__).resolve().parent
PATTERNS_DIR = SCRIPT_DIR / "error_patterns"
MODEL = "gemini-2.5-flash"

def load_error_patterns() -> Dict:
    """Load all error patterns from the analysis."""
    if not PATTERNS_DIR.exists():
        sys.exit("Error patterns not found. Run analyze_transcript_pairs.py first.")

    patterns = {}

    pattern_files = [
        "substitutions.json",
        "punctuation.json",
        "domain_terms.json",
        "length_mismatches.json"
    ]

    for file in pattern_files:
        file_path = PATTERNS_DIR / file
        if file_path.exists():
            try:
                with open(file_path) as f:
                    patterns[file.replace('.json', '')] = json.load(f)
            except json.JSONDecodeError:
                print(f"Warning: Could not load {file}")
                patterns[file.replace('.json', '')] = []

    return patterns

def calculate_similarity(transcript: str, example: Dict) -> float:
    """Calculate similarity score between transcript and error example."""
    transcript_words = set(transcript.lower().split())

    # For substitutions, check context overlap
    if "context_before" in example and "context_after" in example:
        context_words = set(example["context_before"].split() + example["context_after"].split())
        overlap = len(transcript_words & context_words)
        return overlap / max(len(context_words), 1)

    # For length mismatches, check if similar transcript length
    if "groq" in example:
        example_words = set(example["groq"].lower().split())
        overlap = len(transcript_words & example_words)
        return overlap / max(len(example_words), 1)

    # For domain terms, check if the term appears in transcript
    if "correct" in example:
        # Check if either the wrong or correct term appears
        wrong_term = example.get("groq_word", "").lower()
        correct_term = example.get("correct", "").lower()
        if wrong_term in transcript.lower() or correct_term in transcript.lower():
            return 1.0

    return 0.0

def select_relevant_examples(transcript: str, patterns: Dict, max_examples: int = 5) -> List[Tuple[str, str]]:
    """Select the most relevant error correction examples for the transcript."""
    examples = []

    # Collect all potential examples with similarity scores
    candidates = []

    # From substitutions
    for sub in patterns.get("substitutions", []):
        similarity = calculate_similarity(transcript, sub)
        if similarity > 0:
            wrong = sub["groq_word"]
            correct = sub["meta_word"]
            context = f"{sub.get('context_before', '')} {wrong} {sub.get('context_after', '')}".strip()
            correct_context = f"{sub.get('context_before', '')} {correct} {sub.get('context_after', '')}".strip()

            candidates.append((
                similarity,
                f"WRONG: \"{context}\"",
                f"RIGHT: \"{correct_context}\""
            ))

    # From domain terms (high-confidence corrections)
    for wrong_term, info in patterns.get("domain_terms", {}).items():
        if wrong_term.lower() in transcript.lower():
            candidates.append((
                2.0,  # High priority for domain terms
                f"WRONG: \"{wrong_term}\"",
                f"RIGHT: \"{info['correct']}\""
            ))

    # From length mismatches (for context about missing/extra content)
    for mismatch in patterns.get("length_mismatches", []):
        similarity = calculate_similarity(transcript, mismatch)
        if similarity > 0.3:
            candidates.append((
                similarity,
                f"WRONG: \"{mismatch['groq']}\"",
                f"RIGHT: \"{mismatch['meta']}\""
            ))

    # Sort by similarity and take top examples
    candidates.sort(key=lambda x: x[0], reverse=True)

    seen_patterns = set()
    for similarity, wrong, right in candidates[:max_examples * 2]:  # Get more to filter duplicates
        # Avoid near-duplicate examples
        pattern_key = (wrong.lower()[:50], right.lower()[:50])
        if pattern_key not in seen_patterns:
            examples.append((wrong, right))
            seen_patterns.add(pattern_key)

        if len(examples) >= max_examples:
            break

    return examples

def build_improvement_prompt(transcript: str, patterns: Dict) -> str:
    """Build a prompt with relevant examples to improve the transcript."""
    examples = select_relevant_examples(transcript, patterns, max_examples=5)

    if not examples:
        # Fallback to general correction prompt
        return f"""Fix any speech-to-text errors in this transcript. Common issues include:
- Homophones (there/their/they're)
- Technical terms mistranscribed as common words
- Missing or incorrect punctuation
- Proper nouns spelled incorrectly

Only fix obvious errors. Keep the same meaning and tone.

Transcript to fix: "{transcript}"

Return only the corrected transcript."""

    examples_text = "\n\n".join([f"{wrong}\n{right}" for wrong, right in examples])

    return f"""Fix speech-to-text errors in transcripts. Here are examples of common corrections for this speaker:

{examples_text}

Rules:
- Only fix obvious errors like the examples above
- Keep the same meaning and tone
- Don't add words that weren't spoken
- Fix technical terms, homophones, and punctuation
- Preserve the speaker's informal style

Transcript to fix: "{transcript}"

Return only the corrected transcript."""

async def improve_transcript(transcript: str, patterns: Dict) -> str:
    """Improve a transcript using learned error patterns."""
    api_key = os.environ.get("GEMINI_API_KEY")
    if not api_key:
        sys.exit("ERROR: GEMINI_API_KEY not set")

    client = genai.Client(api_key=api_key)
    prompt = build_improvement_prompt(transcript, patterns)

    try:
        resp = await client.aio.models.generate_content(
            model=MODEL,
            contents=prompt,
            config=types.GenerateContentConfig(
                temperature=0.1,  # Low temperature for consistent corrections
                max_output_tokens=2048,
                thinking_config=types.ThinkingConfig(thinking_budget=0),
            ),
        )
        return resp.text.strip()
    except Exception as e:
        print(f"Error improving transcript: {e}", file=sys.stderr)
        return transcript  # Return original on error

def main():
    parser = argparse.ArgumentParser(description="Improve Groq transcripts using learned patterns")
    parser.add_argument("input", help="Transcript text or file path")
    parser.add_argument("--file", action="store_true", help="Input is a file path")
    parser.add_argument("--output", help="Output file (default: stdout)")
    args = parser.parse_args()

    # Load error patterns
    patterns = load_error_patterns()

    # Get input transcript
    if args.file:
        try:
            transcript = Path(args.input).read_text().strip()
        except FileNotFoundError:
            sys.exit(f"File not found: {args.input}")
    else:
        transcript = args.input.strip()

    if not transcript:
        sys.exit("Empty transcript provided")

    # Improve transcript
    import asyncio
    improved = asyncio.run(improve_transcript(transcript, patterns))

    # Output result
    if args.output:
        Path(args.output).write_text(improved)
        print(f"Improved transcript saved to: {args.output}")
    else:
        print(improved)

if __name__ == "__main__":
    main()