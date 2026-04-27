#!/usr/bin/env python3
import json
import os
import shutil

script_dir = os.path.dirname(os.path.abspath(__file__))
recordings_dir = os.path.join(script_dir, "recordings")

deleted_count = 0
total_count = 0
empty_count = 0

print("Scanning recordings for empty meta files...")
print(f"Recordings directory: {recordings_dir}")
print()

for recording_id in os.listdir(recordings_dir):
    recording_path = os.path.join(recordings_dir, recording_id)
    
    if not os.path.isdir(recording_path):
        continue
    
    total_count += 1
    meta_path = os.path.join(recording_path, "meta.json")
    
    if not os.path.exists(meta_path):
        print(f"⚠️  No meta.json found in {recording_id}, skipping")
        continue
    
    try:
        with open(meta_path, 'r') as f:
            meta = json.load(f)
        
        # Check if recording is empty
        result = meta.get("result", "")
        segments = meta.get("segments", [])
        speakers = meta.get("speakers", [])
        
        is_empty = (result == "" or result is None) and len(segments) == 0
        
        if is_empty:
            empty_count += 1
            print(f"🗑️  Deleting empty recording: {recording_id}")
            shutil.rmtree(recording_path)
            deleted_count += 1
        else:
            print(f"✓ Keeping recording: {recording_id} (result: {len(result)} chars, segments: {len(segments)})")
    
    except json.JSONDecodeError as e:
        print(f"⚠️  Invalid JSON in {recording_id}: {e}")
    except Exception as e:
        print(f"⚠️  Error processing {recording_id}: {e}")

print()
print("=" * 50)
print("Summary:")
print(f"Total recordings scanned: {total_count}")
print(f"Empty recordings found: {empty_count}")
print(f"Folders deleted: {deleted_count}")
print(f"Recordings kept: {total_count - deleted_count}")
print("=" * 50)
