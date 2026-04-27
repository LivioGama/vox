#!/usr/bin/env python3
import json
import os
import re

script_dir = os.path.dirname(os.path.abspath(__file__))
recordings_dir = os.path.join(script_dir, "playground", "recordings")

# Patterns that indicate challenging speech-to-text scenarios
challenging_patterns = {
    "homophones": [
        r"\b(there|their|they're)\b",
        r"\b(to|too|two)\b",
        r"\b(your|you're)\b",
        r"\b(its|it's)\b",
        r"\b(then|than)\b",
        r"\b(hear|here)\b",
        r"\b(where|were|wear)\b",
        r"\b(accept|except)\b",
        r"\b(affect|effect)\b",
        r"\b(complement|compliment)\b",
        r"\b(brake|break)\b",
        r"\b(piece|peace)\b",
        r"\b(whole|hole)\b",
        r"\b(write|right|wright)\b",
    ],
    "technical_terms": [
        r"\b(API|JSON|XML|HTTP|HTTPS|URL|URI|SQL|NoSQL)\b",
        r"\b(Rust|Python|JavaScript|TypeScript|Go|Java)\b",
        r"\b(Git|GitHub|GitLab|Docker|Kubernetes)\b",
        r"\b(Linux|Unix|macOS|Windows)\b",
        r"\b(CPU|GPU|RAM|SSD|HDD)\b",
        r"\b(backend|frontend|fullstack|full-stack)\b",
        r"\b(database|server|client|request|response)\b",
        r"\b(algorithm|function|variable|parameter)\b",
        r"\b(repository|commit|branch|merge|pull)\b",
    ],
    "numbers_mixed": [
        r"\b\d+\b",  # Numeric digits
        r"\b(one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|twenty)\b",
    ],
    "repeated_words": [
        r"\b(\w+)\s+\1\b",  # Repeated words
    ],
    "similar_sounding": [
        r"\b(recognition|reckonition|recognition)\b",
        r"\b(pronunciation|pronounciation)\b",
        r"\b(specific|pacific)\b",
        r"\b(especially|specially)\b",
        r"\b(definitely|definately)\b",
    ],
}

ultra_recordings = []
all_ultra_recordings = []  # All recordings, regardless of challenge score
total_duration_seconds = 0

print("Analyzing ultra model recordings for challenging speech-to-text patterns...")
print()

for recording_id in os.listdir(recordings_dir):
    recording_path = os.path.join(recordings_dir, recording_id)
    
    if not os.path.isdir(recording_path):
        continue
    
    meta_path = os.path.join(recording_path, "meta.json")
    
    if not os.path.exists(meta_path):
        continue
    
    try:
        with open(meta_path, 'r') as f:
            meta = json.load(f)
        
        # Filter for ultra model only
        model_key = meta.get("modelKey", "").lower()
        model_name = meta.get("modelName", "").lower()
        
        if "ultra" not in model_key and "ultra" not in model_name:
            continue
        
        result = meta.get("result", "")
        segments = meta.get("segments", [])
        duration_ms = meta.get("duration", 0)
        duration = duration_ms / 1000  # Duration is in milliseconds
        
        if not result or len(result) < 10:
            continue
        
        total_duration_seconds += duration
        
        # Add to all recordings list
        all_ultra_recordings.append({
            "id": recording_id,
            "text": result,
            "duration": duration,
            "segments_count": len(segments),
            "model_name": meta.get("modelName", ""),
        })
        
        # Calculate challenge score
        challenge_score = 0
        reasons = []
        
        # Check homophones
        homophone_count = 0
        for pattern in challenging_patterns["homophones"]:
            matches = re.findall(pattern, result, re.IGNORECASE)
            homophone_count += len(matches)
        if homophone_count > 0:
            challenge_score += homophone_count * 2
            reasons.append(f"Homophones: {homophone_count}")
        
        # Check technical terms
        tech_count = 0
        for pattern in challenging_patterns["technical_terms"]:
            matches = re.findall(pattern, result, re.IGNORECASE)
            tech_count += len(matches)
        if tech_count > 0:
            challenge_score += tech_count * 3
            reasons.append(f"Technical terms: {tech_count}")
        
        # Check mixed numbers
        num_count = len(re.findall(challenging_patterns["numbers_mixed"][0], result))
        word_num_count = len(re.findall(challenging_patterns["numbers_mixed"][1], result, re.IGNORECASE))
        if num_count > 0 or word_num_count > 0:
            challenge_score += (num_count + word_num_count) * 1.5
            reasons.append(f"Numbers: {num_count + word_num_count}")
        
        # Check repeated words
        repeated = re.findall(challenging_patterns["repeated_words"][0], result, re.IGNORECASE)
        if repeated:
            challenge_score += len(repeated) * 4
            reasons.append(f"Repeated words: {len(repeated)}")
        
        # Check for low confidence segments
        low_confidence_count = 0
        for seg in segments:
            if "confidence" in seg and seg["confidence"] < 0.8:
                low_confidence_count += 1
        if low_confidence_count > 0:
            challenge_score += low_confidence_count * 5
            reasons.append(f"Low confidence segments: {low_confidence_count}")
        
        # Check for long/complex words
        words = result.split()
        complex_words = [w for w in words if len(w) > 15]
        if complex_words:
            challenge_score += len(complex_words) * 2
            reasons.append(f"Complex words: {len(complex_words)}")
        
        # Check segment count (more segments = more complex speech)
        if len(segments) > 10:
            challenge_score += len(segments) * 0.5
            reasons.append(f"Many segments: {len(segments)}")
        
        if challenge_score > 0:
            ultra_recordings.append({
                "id": recording_id,
                "text": result,
                "score": challenge_score,
                "reasons": reasons,
                "duration": duration,
                "segments_count": len(segments),
                "model_name": meta.get("modelName", ""),
            })
    
    except Exception as e:
        continue

# Sort by duration only to get longest recordings
all_ultra_recordings.sort(key=lambda x: x["duration"], reverse=True)
ultra_recordings.sort(key=lambda x: (x["score"], x["duration"]), reverse=True)

# Groq pricing (https://groq.com/pricing): per-hour, with a 10s minimum bill per request.
GROQ_LARGE_V3_PER_HOUR = 0.111
GROQ_LARGE_V3_TURBO_PER_HOUR = 0.04
MIN_BILLED_SECONDS = 10

def groq_cost(durations_seconds, per_hour):
    billed_hours = sum(max(d, MIN_BILLED_SECONDS) for d in durations_seconds) / 3600
    return billed_hours * per_hour

all_durations = [r["duration"] for r in all_ultra_recordings]
total_minutes = total_duration_seconds / 60
total_hours = total_minutes / 60

print("=" * 80)
print("ALL ULTRA MODEL RECORDINGS TOTAL DURATION")
print("=" * 80)
print(f"Total ultra model recordings: {len(all_ultra_recordings)}")
print(f"Sum of all_ultra_recordings durations: {sum(all_durations):,.0f} seconds")
print(f"Total duration from counter: {total_duration_seconds:,.0f} seconds")
print(f"Total duration: {total_minutes:,.2f} minutes")
print(f"Total duration: {total_hours:,.2f} hours")
print(f"Average duration per recording: {total_minutes / len(all_ultra_recordings):.2f} minutes")
print()
print(f"Cost (Whisper Large V3,       ${GROQ_LARGE_V3_PER_HOUR}/hr): ${groq_cost(all_durations, GROQ_LARGE_V3_PER_HOUR):,.2f}")
print(f"Cost (Whisper Large V3 Turbo, ${GROQ_LARGE_V3_TURBO_PER_HOUR}/hr): ${groq_cost(all_durations, GROQ_LARGE_V3_TURBO_PER_HOUR):,.2f}")
print("=" * 80)
print()
print("TOP 20 MOST CHALLENGING RECORDINGS (Speech-to-Text Failure Candidates):")
print("=" * 80)
print()

for i, example in enumerate(ultra_recordings[:20], 1):
    print(f"{i}. Recording ID: {example['id']}")
    print(f"   Score: {example['score']:.1f} | Duration: {example['duration']}s | Segments: {example['segments_count']}")
    print(f"   Model: {example['model_name']}")
    print(f"   Reasons: {', '.join(example['reasons'])}")
    print(f"   Text: {example['text'][:200]}{'...' if len(example['text']) > 200 else ''}")
    print()

print("=" * 80)
print(f"Total challenging examples found: {len(ultra_recordings)}")
print()
print("DURATION & COST BY TOP-N LONGEST RECORDINGS (All Ultra Model):")
print(f"(Groq pricing, {MIN_BILLED_SECONDS}s minimum bill per request)")
print("=" * 80)
print(f"{'Bucket':<10} {'Recordings':>10} {'Minutes':>10} {'Hours':>8} {'Large V3':>10} {'V3 Turbo':>10}")
buckets = [20, 200, 500, 1000, len(all_ultra_recordings)]
labels = {20: "Top 20", 200: "Top 200", 500: "Top 500", 1000: "Top 1000", len(all_ultra_recordings): "ALL"}
for n in buckets:
    durs = all_durations[:n]
    secs = sum(durs)
    mins = secs / 60
    hrs = secs / 3600
    cost_v3 = groq_cost(durs, GROQ_LARGE_V3_PER_HOUR)
    cost_turbo = groq_cost(durs, GROQ_LARGE_V3_TURBO_PER_HOUR)
    print(f"{labels[n]:<10} {n:>10} {mins:>10.2f} {hrs:>8.2f} {'$'+f'{cost_v3:.2f}':>10} {'$'+f'{cost_turbo:.2f}':>10}")
print("=" * 80)
