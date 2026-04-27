#!/bin/bash
# Restart vox always mode (build is done by cargo watch)
pkill -f "vox always"
./target/release/vox always start
# Play sound when build/restart completes
./target/release/vox pack play complete
