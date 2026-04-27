#!/usr/bin/env python3
import json
import os
import time
from groq import Groq
import glob

# Configuration
script_dir = os.path.dirname(os.path.abspath(__file__))
recordings_dir = os.path.join(script_dir, "playground", "recordings")
output_dir = os.path.join(script_dir, "playground", "groq_transcripts")

# Groq API rate limits (adjust based on your tier)
# Standard tier: 30 requests per minute
REQUESTS_PER_MINUTE = 30
MIN_REQUEST_INTERVAL = 60 / REQUESTS_PER_MINUTE  # seconds between requests

# Create output directory if it doesn't exist
os.makedirs(output_dir, exist_ok=True)

# Initialize Groq client
client = Groq(api_key=os.environ.get("GROQ_API_KEY"))

def transcribe_recording(audio_file_path, recording_id):
    """Transcribe a single recording using Groq Whisper Large"""
    try:
        with open(audio_file_path, "rb") as audio_file:
            transcription = client.audio.transcriptions.create(
                file=(os.path.basename(audio_file_path), audio_file),
                model="whisper-large-v3",
                response_format="json"
            )
        
        return {
            "recording_id": recording_id,
            "text": transcription.text,
            "segments": transcription.segments if hasattr(transcription, 'segments') else None,
            "language": transcription.language if hasattr(transcription, 'language') else None,
            "duration": transcription.duration if hasattr(transcription, 'duration') else None
        }
    except Exception as e:
        print(f"Error transcribing {recording_id}: {e}")
        return None

def main():
    print("Starting Groq Whisper Large transcription...")
    print(f"Recordings directory: {recordings_dir}")
    print(f"Output directory: {output_dir}")
    print(f"Rate limit: {REQUESTS_PER_MINUTE} requests/minute ({MIN_REQUEST_INTERVAL:.2f}s interval)")
    print()
    
    # Get all recording IDs
    recording_ids = [d for d in os.listdir(recordings_dir) if os.path.isdir(os.path.join(recordings_dir, d))]
    recording_ids.sort()
    
    print(f"Found {len(recording_ids)} recordings to transcribe")
    print()
    
    processed = 0
    skipped = 0
    failed = 0
    
    for i, recording_id in enumerate(recording_ids):
        recording_path = os.path.join(recordings_dir, recording_id)
        output_file = os.path.join(output_dir, f"{recording_id}.json")
        
        # Skip if already transcribed
        if os.path.exists(output_file):
            print(f"✓ Skipping {recording_id} (already transcribed)")
            skipped += 1
            continue
        
        # Find audio file
        audio_files = glob.glob(os.path.join(recording_path, "*.wav")) + glob.glob(os.path.join(recording_path, "*.mp3")) + glob.glob(os.path.join(recording_path, "*.m4a"))
        
        if not audio_files:
            print(f"⚠ No audio file found for {recording_id}, skipping")
            skipped += 1
            continue
        
        audio_file = audio_files[0]
        
        # Rate limiting
        if i > 0 and i % REQUESTS_PER_MINUTE == 0:
            print(f"\n--- Pausing for rate limit (60 seconds) ---\n")
            time.sleep(60)
        elif i > 0:
            time.sleep(MIN_REQUEST_INTERVAL)
        
        print(f"[{i+1}/{len(recording_ids)}] Transcribing {recording_id}...")
        
        # Transcribe
        result = transcribe_recording(audio_file, recording_id)
        
        if result:
            # Save result
            with open(output_file, 'w') as f:
                json.dump(result, f, indent=2)
            print(f"✓ Saved transcription to {output_file}")
            processed += 1
        else:
            failed += 1
    
    print()
    print("=" * 60)
    print("SUMMARY")
    print("=" * 60)
    print(f"Total recordings: {len(recording_ids)}")
    print(f"Processed: {processed}")
    print(f"Skipped: {skipped}")
    print(f"Failed: {failed}")
    print("=" * 60)

if __name__ == "__main__":
    main()
