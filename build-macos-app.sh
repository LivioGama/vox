#!/bin/bash
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
APP_NAME="vox"
APP_BUNDLE="$APP_NAME.app"
# Use ~/Applications instead of /Applications to avoid sudo
USER_APPS="$HOME/Applications"
APP_PATH="$USER_APPS/$APP_BUNDLE"
BINARY_NAME="vox"

echo -e "${BLUE}Building vox macOS App Bundle...${NC}"

# Step 1: Build in release mode with optimizations
echo -e "${YELLOW}Step 1: Building release binary...${NC}"
cargo build --profile release-fast || {
    echo -e "${RED}Failed to build release binary${NC}"
    exit 1
}

# Step 2: Create app bundle structure
echo -e "${YELLOW}Step 2: Creating app bundle structure...${NC}"
TEMP_APP="/tmp/$APP_BUNDLE"
rm -rf "$TEMP_APP"
mkdir -p "$TEMP_APP/Contents/MacOS"
mkdir -p "$TEMP_APP/Contents/Resources"

# Step 3: Copy binary to bundle
echo -e "${YELLOW}Step 3: Installing binary to bundle...${NC}"
cp "target/release-fast/$BINARY_NAME" "$TEMP_APP/Contents/MacOS/$BINARY_NAME"

# Step 4: Create Info.plist
echo -e "${YELLOW}Step 4: Creating Info.plist...${NC}"
cat > "$TEMP_APP/Contents/Info.plist" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDisplayName</key>
    <string>vox</string>
    <key>CFBundleExecutable</key>
    <string>vox</string>
    <key>CFBundleIdentifier</key>
    <string>com.rtkai.vox</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>vox</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.13.0</string>
    <key>CFBundleVersion</key>
    <string>13</string>
    <key>LSBackgroundOnly</key>
    <true/>
    <key>LSMinimumSystemVersion</key>
    <string>10.14.0</string>
    <key>NSMicrophoneUsageDescription</key>
    <string>vox requires microphone access for voice transcription</string>
    <key>NSHumanReadableCopyright</key>
    <string>Copyright © 2024 RTK AI. Licensed under Apache 2.0.</string>
</dict>
</plist>
EOF

# Step 5: Set proper permissions (crucial for avoiding re-prompts)
echo -e "${YELLOW}Step 5: Setting permissions...${NC}"
chmod +x "$TEMP_APP/Contents/MacOS/$BINARY_NAME"
chmod -R 755 "$TEMP_APP"

# Step 6: Remove extended attributes that might cause issues
echo -e "${YELLOW}Step 6: Removing extended attributes...${NC}"
xattr -cr "$TEMP_APP" 2>/dev/null || true

# Step 7: Install to ~/Applications with proper permissions preservation
echo -e "${YELLOW}Step 7: Installing to ~/Applications...${NC}"

# Create ~/Applications if it doesn't exist
mkdir -p "$USER_APPS"

if [[ -d "$APP_PATH" ]]; then
    echo "Removing existing app bundle..."
    rm -rf "$APP_PATH"
fi

# Use rsync to preserve permissions and attributes properly
rsync -a "$TEMP_APP/" "$APP_PATH/"

# Set proper permissions without sudo
chmod -R 755 "$APP_PATH"
chmod +x "$APP_PATH/Contents/MacOS/$BINARY_NAME"

# Step 8: Create command line alias script
echo -e "${YELLOW}Step 8: Creating command-line wrapper...${NC}"
WRAPPER_PATH="/usr/local/bin/$BINARY_NAME"

# Create local bin directory if it doesn't exist
LOCAL_BIN="$HOME/.local/bin"
mkdir -p "$LOCAL_BIN"
LOCAL_WRAPPER="$LOCAL_BIN/$BINARY_NAME"

# Try to create wrapper in /usr/local/bin first, fall back to ~/.local/bin
if [[ -w "/usr/local/bin" ]]; then
    tee "$WRAPPER_PATH" > /dev/null << EOF
#!/bin/bash
exec "$APP_PATH/Contents/MacOS/$BINARY_NAME" "\$@"
EOF
    chmod +x "$WRAPPER_PATH"
    FINAL_WRAPPER="$WRAPPER_PATH"
else
    # Fall back to local bin
    tee "$LOCAL_WRAPPER" > /dev/null << EOF
#!/bin/bash
exec "$APP_PATH/Contents/MacOS/$BINARY_NAME" "\$@"
EOF
    chmod +x "$LOCAL_WRAPPER"
    FINAL_WRAPPER="$LOCAL_WRAPPER"
    echo -e "${YELLOW}Note: Created wrapper in ~/.local/bin instead of /usr/local/bin${NC}"
    echo -e "${YELLOW}Add ~/.local/bin to your PATH if needed${NC}"
fi

# Step 9: Verify installation
echo -e "${YELLOW}Step 9: Verifying installation...${NC}"
if [[ -x "$APP_PATH/Contents/MacOS/$BINARY_NAME" ]]; then
    echo -e "${GREEN}✓ App bundle created successfully at $APP_PATH${NC}"
else
    echo -e "${RED}✗ App bundle verification failed${NC}"
    exit 1
fi

# Test that the wrapper works
if command -v "$BINARY_NAME" >/dev/null 2>&1; then
    echo -e "${GREEN}✓ Command-line wrapper created at $FINAL_WRAPPER${NC}"
else
    echo -e "${YELLOW}⚠ Command-line wrapper may not be in PATH: $FINAL_WRAPPER${NC}"
fi

# Cleanup
rm -rf "$TEMP_APP"

echo -e "${GREEN}vox app bundle installation complete!${NC}"
echo
echo -e "${BLUE}Usage:${NC}"
echo "  • App bundle: $APP_PATH"
echo "  • Command line: $BINARY_NAME (via $FINAL_WRAPPER)"
echo "  • Binary location: $APP_PATH/Contents/MacOS/$BINARY_NAME"
echo
echo -e "${BLUE}Next steps:${NC}"
echo "  1. Grant microphone permissions if prompted"
echo "  2. Test with: $BINARY_NAME --help"
echo "  3. Start daemon: $BINARY_NAME always start"