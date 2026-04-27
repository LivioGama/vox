#!/bin/bash

# Build without touching the running process
cargo build --release

# Only if build succeeded, then kill and restart
if [ $? -eq 0 ]; then
    pkill -f 'vox always'
    sleep 1
    ./target/release/vox always start
fi
