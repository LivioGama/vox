#!/usr/bin/env python3
import json
import os

script_dir = os.path.dirname(os.path.abspath(__file__))
recordings_dir = os.path.join(script_dir, "playground", "recordings")

total_duration_seconds = 0
total_recordings = 0

print("Calculating total duration of recordings...")
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
        
        duration = meta.get("duration", 0)
        total_duration_seconds += duration
        total_recordings += 1
    
    except Exception as e:
        continue

total_minutes = total_duration_seconds / 60
total_hours = total_minutes / 60

print("=" * 60)
print("DURATION SUMMARY")
print("=" * 60)
print(f"Total recordings: {total_recordings}")
print(f"Total duration: {total_duration_seconds:,.0f} seconds")
print(f"Total duration: {total_minutes:,.2f} minutes")
print(f"Total duration: {total_hours:,.2f} hours")
print("=" * 60)
print()
print("GROQ WHISPER LARGE V3 PRICING ESTIMATE")
print("=" * 60)
print("Note: Please verify current pricing at https://console.groq.com/docs/pricing")
print()
print("As of recent pricing:")
print("- Whisper Large V3: $0.006 per minute")
print()
estimated_cost = total_minutes * 0.006
print(f"Estimated cost: ${estimated_cost:,.2f}")
print("=" * 60)
