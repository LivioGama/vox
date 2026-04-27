#!/usr/bin/env python3
import json
import os
import re
from collections import defaultdict

script_dir = os.path.dirname(os.path.abspath(__file__))
recordings_dir = os.path.join(script_dir, "playground", "recordings")

# Common speech-to-text mistake patterns and interesting patterns
patterns_to_check = {
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
    ],
    "technical_terms": [
        r"\b(API|JSON|XML|HTTP|HTTPS|URL|URI|SQL|NoSQL)\b",
        r"\b(Rust|Python|JavaScript|TypeScript|Go|Java)\b",
        r"\b(Git|GitHub|GitLab|Docker|Kubernetes)\b",
        r"\b(Linux|Unix|macOS|Windows)\b",
        r"\b(CPU|GPU|RAM|SSD|HDD)\b",
        r"\b(backend|frontend|fullstack|full-stack)\b",
        r"\b(database|server|client|request|response)\b",
    ],
    "numbers": [
        r"\b\d+\b",
        r"\b(one|two|three|four|five|six|seven|eight|nine|ten)\b",
    ],
    "capitalization": [
        r"\b[A-Z][a-z]+\s+[A-Z][a-z]+\b",  # Proper nouns
    ],
    "punctuation": [
        r"[!?]{2,}",  # Multiple punctuation
    ],
    "repeated_words": [
        r"\b(\w+)\s+\1\b",  # Repeated words
    ],
}

interesting_examples = []
total_checked = 0

print("Analyzing recordings for interesting patterns...")
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
        
        result = meta.get("result", "")
        segments = meta.get("segments", [])
        
        if not result or len(result) < 10:
            continue
        
        total_checked += 1
        
        # Check for interesting patterns
        reasons = []
        
        # Check for homophones
        for pattern in patterns_to_check["homophones"]:
            if re.search(pattern, result, re.IGNORECASE):
                matches = re.findall(pattern, result, re.IGNORECASE)
                reasons.append(f"Homophone: {matches[0]}")
        
        # Check for technical terms
        for pattern in patterns_to_check["technical_terms"]:
            if re.search(pattern, result, re.IGNORECASE):
                matches = re.findall(pattern, result, re.IGNORECASE)
                reasons.append(f"Technical: {matches[0]}")
        
        # Check for numbers
        if re.search(patterns_to_check["numbers"][0], result):
            numbers = re.findall(patterns_to_check["numbers"][0], result)
            reasons.append(f"Numbers: {numbers[:3]}")
        
        # Check for repeated words
        if re.search(patterns_to_check["repeated_words"][0], result, re.IGNORECASE):
            repeated = re.findall(patterns_to_check["repeated_words"][0], result, re.IGNORECASE)
            reasons.append(f"Repeated: {repeated[0]}")
        
        # Check for long words (might be technical or complex)
        words = result.split()
        long_words = [w for w in words if len(w) > 12]
        if long_words:
            reasons.append(f"Long words: {long_words[:2]}")
        
        # Check for segments with low confidence (if available)
        if segments:
            for seg in segments:
                if "confidence" in seg and seg["confidence"] < 0.7:
                    reasons.append(f"Low confidence: {seg.get('text', '')[:30]}")
                    break
        
        if reasons:
            interesting_examples.append({
                "id": recording_id,
                "text": result,
                "reasons": reasons,
                "duration": meta.get("duration", 0),
                "segments_count": len(segments),
            })
    
    except Exception as e:
        continue

# Sort by number of reasons (most interesting first)
interesting_examples.sort(key=lambda x: len(x["reasons"]), reverse=True)

print(f"Found {len(interesting_examples)} interesting examples out of {total_checked} recordings")
print()
print("=" * 80)
print("TOP 20 INTERESTING RECORDINGS:")
print("=" * 80)
print()

for i, example in enumerate(interesting_examples[:20], 1):
    print(f"{i}. Recording ID: {example['id']}")
    print(f"   Duration: {example['duration']}s | Segments: {example['segments_count']}")
    print(f"   Reasons: {', '.join(example['reasons'])}")
    print(f"   Text: {example['text'][:200]}{'...' if len(example['text']) > 200 else ''}")
    print()

print("=" * 80)
print(f"Total interesting examples found: {len(interesting_examples)}")
