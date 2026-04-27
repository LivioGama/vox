#!/usr/bin/env python3
"""
Analyze Meta (ground truth) vs Groq transcript pairs to extract error patterns.
Creates a database of correction examples for improving future Groq transcripts.
"""
import argparse
import json
import os
import re
from collections import defaultdict
from pathlib import Path
from typing import Dict, List, Tuple

SCRIPT_DIR = Path(__file__).resolve().parent
RECORDINGS_DIR = SCRIPT_DIR / "playground" / "recordings"
GROQ_DIR = SCRIPT_DIR / "playground" / "groq_transcripts"
PATTERNS_DIR = SCRIPT_DIR / "error_patterns"

def load_transcript_pairs() -> List[Tuple[str, str, str, float]]:
    """Load (recording_id, meta_transcript, groq_transcript, duration) pairs."""
    pairs = []

    for groq_file in GROQ_DIR.glob("*.json"):
        recording_id = groq_file.stem
        meta_path = RECORDINGS_DIR / recording_id / "meta.json"
        if not meta_path.exists():
            continue

        try:
            meta_data = json.loads(meta_path.read_text())
            meta_text = (meta_data.get("result") or "").strip()
            duration = meta_data.get("duration", 0) / 1000

            groq_data = json.loads(groq_file.read_text())
            groq_text = (groq_data.get("text") or "").strip()

            if meta_text and groq_text and meta_text != groq_text:
                pairs.append((recording_id, meta_text, groq_text, duration))

        except (json.JSONDecodeError, FileNotFoundError):
            continue

    return pairs

def extract_word_substitutions(meta: str, groq: str) -> List[Dict]:
    """Find word-level substitutions between meta and groq transcripts."""
    # Simple word-by-word alignment (could be improved with proper alignment algorithm)
    meta_words = meta.lower().split()
    groq_words = groq.lower().split()

    substitutions = []

    # For now, find obvious mismatches at same positions
    min_len = min(len(meta_words), len(groq_words))
    for i in range(min_len):
        if meta_words[i] != groq_words[i]:
            # Check if it's a phonetically similar error
            if is_phonetically_similar(meta_words[i], groq_words[i]):
                substitutions.append({
                    "groq_word": groq_words[i],
                    "meta_word": meta_words[i],
                    "position": i,
                    "context_before": " ".join(groq_words[max(0, i-2):i]),
                    "context_after": " ".join(groq_words[i+1:min(len(groq_words), i+3)]),
                })

    return substitutions

def is_phonetically_similar(word1: str, word2: str) -> bool:
    """Basic check for phonetic similarity."""
    if abs(len(word1) - len(word2)) > 3:
        return False

    # Check edit distance
    def edit_distance(s1, s2):
        if len(s1) < len(s2):
            return edit_distance(s2, s1)

        if len(s2) == 0:
            return len(s1)

        prev = list(range(len(s2) + 1))
        for i, c1 in enumerate(s1):
            curr = [i + 1]
            for j, c2 in enumerate(s2):
                insertions = prev[j + 1] + 1
                deletions = curr[j] + 1
                substitutions = prev[j] + (c1 != c2)
                curr.append(min(insertions, deletions, substitutions))
            prev = curr

        return prev[-1]

    return edit_distance(word1, word2) <= max(2, len(word1) // 3)

def extract_punctuation_patterns(meta: str, groq: str) -> List[Dict]:
    """Find punctuation differences."""
    patterns = []

    # Remove words, keep only punctuation and structure
    meta_punct = re.sub(r'[a-zA-Z0-9]+', 'X', meta)
    groq_punct = re.sub(r'[a-zA-Z0-9]+', 'X', groq)

    if meta_punct != groq_punct:
        patterns.append({
            "type": "punctuation_mismatch",
            "meta_pattern": meta_punct,
            "groq_pattern": groq_punct,
            "meta_full": meta,
            "groq_full": groq,
        })

    return patterns

def extract_domain_terms(pairs: List[Tuple]) -> Dict[str, int]:
    """Find technical terms that are frequently mistranscribed."""
    term_errors = defaultdict(lambda: defaultdict(int))

    for _, meta, groq, _ in pairs:
        substitutions = extract_word_substitutions(meta, groq)
        for sub in substitutions:
            term_errors[sub["meta_word"]][sub["groq_word"]] += 1

    # Filter for high-confidence technical term errors
    domain_terms = {}
    for correct_word, errors in term_errors.items():
        if len(correct_word) > 3 and any(count >= 2 for count in errors.values()):
            most_common_error = max(errors.items(), key=lambda x: x[1])
            domain_terms[most_common_error[0]] = {
                "correct": correct_word,
                "frequency": most_common_error[1],
                "alternatives": dict(errors)
            }

    return domain_terms

def find_length_mismatches(pairs: List[Tuple]) -> List[Dict]:
    """Find cases where Groq transcript is much shorter/longer than Meta."""
    mismatches = []

    for recording_id, meta, groq, duration in pairs:
        meta_len = len(meta.split())
        groq_len = len(groq.split())

        if abs(meta_len - groq_len) > max(3, meta_len * 0.2):
            mismatches.append({
                "recording_id": recording_id,
                "meta": meta,
                "groq": groq,
                "duration": duration,
                "meta_words": meta_len,
                "groq_words": groq_len,
                "type": "missing_content" if groq_len < meta_len else "hallucination"
            })

    return mismatches

def analyze_pairs(pairs: List[Tuple]) -> Dict:
    """Extract all error patterns from transcript pairs."""
    print(f"Analyzing {len(pairs)} transcript pairs...")

    all_substitutions = []
    all_punctuation = []

    for recording_id, meta, groq, duration in pairs:
        substitutions = extract_word_substitutions(meta, groq)
        punctuation = extract_punctuation_patterns(meta, groq)

        for sub in substitutions:
            sub["recording_id"] = recording_id
            sub["duration"] = duration

        for punct in punctuation:
            punct["recording_id"] = recording_id
            punct["duration"] = duration

        all_substitutions.extend(substitutions)
        all_punctuation.extend(punctuation)

    domain_terms = extract_domain_terms(pairs)
    length_mismatches = find_length_mismatches(pairs)

    return {
        "substitutions": all_substitutions,
        "punctuation": all_punctuation,
        "domain_terms": domain_terms,
        "length_mismatches": length_mismatches,
        "total_pairs": len(pairs),
    }

def save_patterns(patterns: Dict):
    """Save extracted patterns to JSON files."""
    PATTERNS_DIR.mkdir(exist_ok=True)

    for category, data in patterns.items():
        if category == "total_pairs":
            continue

        output_file = PATTERNS_DIR / f"{category}.json"
        with open(output_file, 'w') as f:
            json.dump(data, f, indent=2)
        print(f"Saved {len(data) if isinstance(data, list) else len(data)} {category} to {output_file}")

    # Save summary
    summary_file = PATTERNS_DIR / "summary.json"
    summary = {
        "total_pairs_analyzed": patterns["total_pairs"],
        "substitution_errors": len(patterns["substitutions"]),
        "punctuation_errors": len(patterns["punctuation"]),
        "domain_term_errors": len(patterns["domain_terms"]),
        "length_mismatches": len(patterns["length_mismatches"]),
    }

    with open(summary_file, 'w') as f:
        json.dump(summary, f, indent=2)
    print(f"Saved analysis summary to {summary_file}")

def main():
    parser = argparse.ArgumentParser(description="Analyze Meta vs Groq transcript pairs")
    args = parser.parse_args()

    pairs = load_transcript_pairs()

    if not pairs:
        print(f"No transcript pairs found. Looked in:")
        print(f"  Meta:  {RECORDINGS_DIR}/<id>/meta.json")
        print(f"  Groq:  {GROQ_DIR}/<id>.json")
        return

    patterns = analyze_pairs(pairs)
    save_patterns(patterns)

    print(f"\nAnalysis complete! Found patterns in {len(pairs)} transcript pairs.")
    print(f"Error patterns saved to: {PATTERNS_DIR}")

if __name__ == "__main__":
    main()