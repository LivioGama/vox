#!/usr/bin/env python3
import json
import os
import shutil

script_dir = os.path.dirname(os.path.abspath(__file__))
recordings_dir = os.path.join(script_dir, "playground", "recordings")

deleted_count = 0
kept_count = 0
total_count = 0

print("Scanning for non-ultra model recordings to delete...")
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
        
        # Check if it's an ultra model
        model_key = meta.get("modelKey", "").lower()
        model_name = meta.get("modelName", "").lower()
        
        is_ultra = "ultra" in model_key or "ultra" in model_name
        
        if not is_ultra:
            print(f"🗑️  Deleting non-ultra recording: {recording_id} (model: {meta.get('modelName', 'unknown')})")
            shutil.rmtree(recording_path)
            deleted_count += 1
        else:
            kept_count += 1
    
    except Exception as e:
        print(f"⚠️  Error processing {recording_id}: {e}")
        continue

print()
print("=" * 60)
print("SUMMARY")
print("=" * 60)
print(f"Total recordings scanned: {total_count}")
print(f"Deleted (non-ultra): {deleted_count}")
print(f"Kept (ultra): {kept_count}")
print("=" * 60)
