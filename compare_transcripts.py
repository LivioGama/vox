#!/usr/bin/env python3
"""
Compare meta (perfect) transcripts with Groq Whisper Large v3 transcripts.
Analyzes differences to identify patterns for improving Groq transcript quality.
Optimized for processing 600+ transcripts efficiently.
"""

import json
import os
import difflib
from pathlib import Path
from concurrent.futures import ProcessPoolExecutor, as_completed
from typing import Dict, List, Tuple, Optional
import argparse
from dataclasses import dataclass, asdict
from collections import Counter
import re

# Configuration
SCRIPT_DIR = Path(__file__).parent.absolute()
RECORDINGS_DIR = SCRIPT_DIR / "playground" / "recordings"
GROQ_TRANSCRIPTS_DIR = SCRIPT_DIR / "playground" / "groq_transcripts"
OUTPUT_DIR = SCRIPT_DIR / "playground" / "comparison_results"

# Create output directory
OUTPUT_DIR.mkdir(exist_ok=True)


@dataclass
class TranscriptComparison:
    recording_id: str
    meta_text: str
    groq_text: str
    char_diff: int
    word_count_meta: int
    word_count_groq: int
    word_diff: int
    wer: float  # Word Error Rate
    cer: float  # Character Error Rate
    added_text: List[str]
    removed_text: List[str]
    common_text: List[str]
    has_meta: bool
    has_groq: bool


def normalize_text(text: str) -> str:
    """Normalize text for comparison: lowercase, remove extra whitespace, normalize punctuation."""
    if not text:
        return ""
    # Convert to lowercase
    text = text.lower()
    # Remove extra whitespace
    text = re.sub(r'\s+', ' ', text).strip()
    # Normalize common punctuation differences
    text = text.replace('"', "'").replace('"', "'")
    return text


def calculate_wer(reference: str, hypothesis: str) -> float:
    """Calculate Word Error Rate (WER)."""
    if not reference:
        return 1.0 if hypothesis else 0.0
    if not hypothesis:
        return 1.0
    
    ref_words = reference.split()
    hyp_words = hypothesis.split()
    
    # Levenshtein distance for words
    m, n = len(ref_words), len(hyp_words)
    dp = [[0] * (n + 1) for _ in range(m + 1)]
    
    for i in range(m + 1):
        dp[i][0] = i
    for j in range(n + 1):
        dp[0][j] = j
    
    for i in range(1, m + 1):
        for j in range(1, n + 1):
            if ref_words[i - 1] == hyp_words[j - 1]:
                dp[i][j] = dp[i - 1][j - 1]
            else:
                dp[i][j] = 1 + min(dp[i - 1][j], dp[i][j - 1], dp[i - 1][j - 1])
    
    return dp[m][n] / len(ref_words)


def calculate_cer(reference: str, hypothesis: str) -> float:
    """Calculate Character Error Rate (CER)."""
    if not reference:
        return 1.0 if hypothesis else 0.0
    if not hypothesis:
        return 1.0
    
    # Levenshtein distance for characters
    m, n = len(reference), len(hypothesis)
    dp = [[0] * (n + 1) for _ in range(m + 1)]
    
    for i in range(m + 1):
        dp[i][0] = i
    for j in range(n + 1):
        dp[0][j] = j
    
    for i in range(1, m + 1):
        for j in range(1, n + 1):
            if reference[i - 1] == hypothesis[j - 1]:
                dp[i][j] = dp[i - 1][j - 1]
            else:
                dp[i][j] = 1 + min(dp[i - 1][j], dp[i][j - 1], dp[i - 1][j - 1])
    
    return dp[m][n] / len(reference)


def get_text_diff(meta_text: str, groq_text: str) -> Tuple[List[str], List[str], List[str]]:
    """Get added, removed, and common text segments using difflib."""
    meta_words = meta_text.split()
    groq_words = groq_text.split()
    
    matcher = difflib.SequenceMatcher(None, meta_words, groq_words)
    
    added = []
    removed = []
    common = []
    
    for tag, i1, i2, j1, j2 in matcher.get_opcodes():
        if tag == 'replace':
            removed.extend(meta_words[i1:i2])
            added.extend(groq_words[j1:j2])
        elif tag == 'delete':
            removed.extend(meta_words[i1:i2])
        elif tag == 'insert':
            added.extend(groq_words[j1:j2])
        elif tag == 'equal':
            common.extend(meta_words[i1:i2])
    
    return added, removed, common


def compare_single_recording(recording_id: str) -> Optional[TranscriptComparison]:
    """Compare a single recording's meta and groq transcripts."""
    meta_path = RECORDINGS_DIR / recording_id / "meta.json"
    groq_path = GROQ_TRANSCRIPTS_DIR / f"{recording_id}.json"
    
    has_meta = meta_path.exists()
    has_groq = groq_path.exists()
    
    if not has_meta and not has_groq:
        return None
    
    meta_text = ""
    groq_text = ""
    
    if has_meta:
        try:
            with open(meta_path, 'r', encoding='utf-8') as f:
                meta_data = json.load(f)
                # Try different possible keys for transcript text
                meta_text = (
                    meta_data.get('text') or 
                    meta_data.get('transcript') or 
                    meta_data.get('content') or 
                    meta_data.get('rawResult') or
                    (meta_data.get('segments', [{}])[0].get('text') if meta_data.get('segments') else "") or
                    ""
                )
        except Exception as e:
            print(f"Error reading meta for {recording_id}: {e}")
            has_meta = False
    
    if has_groq:
        try:
            with open(groq_path, 'r', encoding='utf-8') as f:
                groq_data = json.load(f)
                groq_text = groq_data.get('text', '')
        except Exception as e:
            print(f"Error reading groq for {recording_id}: {e}")
            has_groq = False
    
    # Normalize texts for comparison
    meta_normalized = normalize_text(meta_text)
    groq_normalized = normalize_text(groq_text)
    
    # Calculate metrics
    char_diff = abs(len(meta_normalized) - len(groq_normalized))
    word_count_meta = len(meta_normalized.split())
    word_count_groq = len(groq_normalized.split())
    word_diff = word_count_meta - word_count_groq
    
    wer = calculate_wer(meta_normalized, groq_normalized)
    cer = calculate_cer(meta_normalized, groq_normalized)
    
    # Get text differences
    added, removed, common = get_text_diff(meta_normalized, groq_normalized)
    
    return TranscriptComparison(
        recording_id=recording_id,
        meta_text=meta_text,
        groq_text=groq_text,
        char_diff=char_diff,
        word_count_meta=word_count_meta,
        word_count_groq=word_count_groq,
        word_diff=word_diff,
        wer=wer,
        cer=cer,
        added_text=added,
        removed_text=removed,
        common_text=common,
        has_meta=has_meta,
        has_groq=has_groq
    )


def analyze_patterns(comparisons: List[TranscriptComparison]) -> Dict:
    """Analyze patterns across all comparisons to identify improvement strategies."""
    total = len(comparisons)
    if total == 0:
        return {}
    
    # Filter to only those with both meta and groq
    valid_comparisons = [c for c in comparisons if c.has_meta and c.has_groq]
    valid_count = len(valid_comparisons)
    
    if valid_count == 0:
        return {"error": "No valid comparisons with both meta and groq transcripts"}
    
    # Calculate average metrics
    avg_wer = sum(c.wer for c in valid_comparisons) / valid_count
    avg_cer = sum(c.cer for c in valid_comparisons) / valid_count
    avg_word_diff = sum(c.word_diff for c in valid_comparisons) / valid_count
    
    # Count common issues
    all_added = []
    all_removed = []
    
    for c in valid_comparisons:
        all_added.extend(c.added_text)
        all_removed.extend(c.removed_text)
    
    # Most common additions/removals
    added_counter = Counter(all_added)
    removed_counter = Counter(all_removed)
    
    # Categorize by WER ranges
    wer_ranges = {
        "excellent (0-0.1)": 0,
        "good (0.1-0.2)": 0,
        "fair (0.2-0.3)": 0,
        "poor (0.3-0.5)": 0,
        "very poor (>0.5)": 0
    }
    
    for c in valid_comparisons:
        if c.wer <= 0.1:
            wer_ranges["excellent (0-0.1)"] += 1
        elif c.wer <= 0.2:
            wer_ranges["good (0.1-0.2)"] += 1
        elif c.wer <= 0.3:
            wer_ranges["fair (0.2-0.3)"] += 1
        elif c.wer <= 0.5:
            wer_ranges["poor (0.3-0.5)"] += 1
        else:
            wer_ranges["very poor (>0.5)"] += 1
    
    return {
        "total_recordings": total,
        "valid_comparisons": valid_count,
        "meta_only": sum(1 for c in comparisons if c.has_meta and not c.has_groq),
        "groq_only": sum(1 for c in comparisons if not c.has_meta and c.has_groq),
        "avg_wer": avg_wer,
        "avg_cer": avg_cer,
        "avg_word_diff": avg_word_diff,
        "wer_distribution": wer_ranges,
        "most_common_additions": added_counter.most_common(20),
        "most_common_removals": removed_counter.most_common(20),
        "improvement_suggestions": generate_improvement_suggestions(avg_wer, avg_cer, avg_word_diff, added_counter, removed_counter)
    }


def generate_improvement_suggestions(avg_wer: float, avg_cer: float, avg_word_diff: float, 
                                    added_counter: Counter, removed_counter: Counter) -> List[str]:
    """Generate suggestions for improving Groq transcript quality."""
    suggestions = []
    
    # Analyze word difference trend
    if avg_word_diff > 5:
        suggestions.append("Groq is consistently missing words - consider checking for audio gaps or silence detection issues")
    elif avg_word_diff < -5:
        suggestions.append("Groq is hallucinating extra words - consider adjusting decoding parameters or temperature")
    
    # Analyze WER
    if avg_wer > 0.3:
        suggestions.append("High Word Error Rate detected - consider post-processing with language models or spell correction")
    elif avg_wer > 0.2:
        suggestions.append("Moderate Word Error Rate - context-aware post-processing may help")
    
    # Analyze common additions (hallucinations)
    common_additions = [word for word, count in added_counter.most_common(10) if count > len(added_counter) * 0.05]
    if common_additions:
        suggestions.append(f"Common hallucinated words detected: {', '.join(common_additions[:5])} - consider adding to stop words list")
    
    # Analyze common removals (missed words)
    common_removals = [word for word, count in removed_counter.most_common(10) if count > len(removed_counter) * 0.05]
    if common_removals:
        suggestions.append(f"Commonly missed words: {', '.join(common_removals[:5])} - may indicate audio quality issues for these phonemes")
    
    if not suggestions:
        suggestions.append("Transcript quality is generally good - minor tweaks to post-processing may yield small improvements")
    
    return suggestions


def main():
    parser = argparse.ArgumentParser(description="Compare meta and Groq transcripts")
    parser.add_argument("--workers", type=int, default=4, help="Number of parallel workers")
    parser.add_argument("--limit", type=int, default=None, help="Limit number of recordings to process")
    parser.add_argument("--detailed", action="store_true", help="Output detailed per-recording comparison")
    args = parser.parse_args()
    
    print("=" * 60)
    print("TRANSCRIPT COMPARISON: Meta vs Groq Whisper Large v3")
    print("=" * 60)
    print(f"Recordings dir: {RECORDINGS_DIR}")
    print(f"Groq transcripts dir: {GROQ_TRANSCRIPTS_DIR}")
    print(f"Output dir: {OUTPUT_DIR}")
    print(f"Parallel workers: {args.workers}")
    print()
    
    # Get all recording IDs
    if not RECORDINGS_DIR.exists():
        print(f"Error: Recordings directory not found: {RECORDINGS_DIR}")
        return
    
    recording_ids = [d.name for d in RECORDINGS_DIR.iterdir() if d.is_dir()]
    recording_ids.sort()
    
    if args.limit:
        recording_ids = recording_ids[:args.limit]
    
    print(f"Found {len(recording_ids)} recordings to process")
    print()
    
    # Process in parallel
    comparisons = []
    print(f"Processing with {args.workers} parallel workers...")
    
    with ProcessPoolExecutor(max_workers=args.workers) as executor:
        future_to_id = {executor.submit(compare_single_recording, rid): rid for rid in recording_ids}
        
        for i, future in enumerate(as_completed(future_to_id), 1):
            rid = future_to_id[future]
            try:
                result = future.result()
                if result:
                    comparisons.append(result)
                    print(f"[{i}/{len(recording_ids)}] ✓ Processed {rid}")
                else:
                    print(f"[{i}/{len(recording_ids)}] ⊘ Skipped {rid} (no transcripts)")
            except Exception as e:
                print(f"[{i}/{len(recording_ids)}] ✗ Error processing {rid}: {e}")
    
    print()
    print("=" * 60)
    print("ANALYSIS")
    print("=" * 60)
    
    # Analyze patterns
    analysis = analyze_patterns(comparisons)
    
    if "error" in analysis:
        print(analysis["error"])
        return
    
    print(f"\nTotal recordings: {analysis['total_recordings']}")
    print(f"Valid comparisons (both meta & groq): {analysis['valid_comparisons']}")
    print(f"Meta only: {analysis['meta_only']}")
    print(f"Groq only: {analysis['groq_only']}")
    print()
    print(f"Average Word Error Rate (WER): {analysis['avg_wer']:.4f}")
    print(f"Average Character Error Rate (CER): {analysis['avg_cer']:.4f}")
    print(f"Average word difference (meta - groq): {analysis['avg_word_diff']:.2f}")
    print()
    print("WER Distribution:")
    for range_name, count in analysis['wer_distribution'].items():
        percentage = (count / analysis['valid_comparisons']) * 100 if analysis['valid_comparisons'] > 0 else 0
        print(f"  {range_name}: {count} ({percentage:.1f}%)")
    print()
    print("Most Common Hallucinated Words (Groq adds):")
    for word, count in analysis['most_common_additions'][:10]:
        print(f"  '{word}': {count} times")
    print()
    print("Most Common Missed Words (Groq removes):")
    for word, count in analysis['most_common_removals'][:10]:
        print(f"  '{word}': {count} times")
    print()
    print("IMPROVEMENT SUGGESTIONS:")
    for i, suggestion in enumerate(analysis['improvement_suggestions'], 1):
        print(f"  {i}. {suggestion}")
    
    # Save results
    results_file = OUTPUT_DIR / "comparison_summary.json"
    with open(results_file, 'w', encoding='utf-8') as f:
        json.dump(analysis, f, indent=2)
    print()
    print(f"✓ Summary saved to {results_file}")
    
    # Save detailed comparisons if requested
    if args.detailed:
        detailed_file = OUTPUT_DIR / "detailed_comparisons.json"
        detailed_data = [asdict(c) for c in comparisons]
        with open(detailed_file, 'w', encoding='utf-8') as f:
            json.dump(detailed_data, f, indent=2)
        print(f"✓ Detailed comparisons saved to {detailed_file}")
    
    print()
    print("=" * 60)


if __name__ == "__main__":
    main()
