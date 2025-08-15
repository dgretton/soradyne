#!/bin/bash
# Copy the Rust library to the app bundle after build

echo "Starting copy_dylib.sh script..."

# Source path for the dylib
DYLIB_SOURCE="../../../packages/soradyne_core/target/release/libsoradyne.dylib"

# Destination paths for both Debug and Release builds
DEBUG_APP_BUNDLE="build/macos/Build/Products/Debug/soradyne_app.app/Contents/MacOS/"
DEBUG_FRAMEWORKS_DIR="build/macos/Build/Products/Debug/soradyne_app.app/Contents/Frameworks/"
RELEASE_APP_BUNDLE="build/macos/Build/Products/Release/soradyne_app.app/Contents/MacOS/"
RELEASE_FRAMEWORKS_DIR="build/macos/Build/Products/Release/soradyne_app.app/Contents/Frameworks/"

echo "Checking for dylib at: $DYLIB_SOURCE"

if [ -f "$DYLIB_SOURCE" ]; then
    echo "Found libsoradyne.dylib, checking destination directories..."
    
    # Copy to Debug build if it exists
    if [ -d "$DEBUG_APP_BUNDLE" ]; then
        echo "Copying to Debug MacOS directory: $DEBUG_APP_BUNDLE"
        cp "$DYLIB_SOURCE" "$DEBUG_APP_BUNDLE"
        echo "Copied libsoradyne.dylib to Debug app bundle MacOS directory"
    else
        echo "Debug MacOS directory not found: $DEBUG_APP_BUNDLE"
    fi
    
    if [ -d "$DEBUG_FRAMEWORKS_DIR" ]; then
        echo "Copying to Debug Frameworks directory: $DEBUG_FRAMEWORKS_DIR"
        cp "$DYLIB_SOURCE" "$DEBUG_FRAMEWORKS_DIR"
        echo "Copied libsoradyne.dylib to Debug app bundle Frameworks directory"
    else
        echo "Debug Frameworks directory not found: $DEBUG_FRAMEWORKS_DIR"
    fi
    
    # Copy to Release build if it exists
    if [ -d "$RELEASE_APP_BUNDLE" ]; then
        echo "Copying to Release MacOS directory: $RELEASE_APP_BUNDLE"
        cp "$DYLIB_SOURCE" "$RELEASE_APP_BUNDLE"
        echo "Copied libsoradyne.dylib to Release app bundle MacOS directory"
    else
        echo "Release MacOS directory not found: $RELEASE_APP_BUNDLE"
    fi
    
    if [ -d "$RELEASE_FRAMEWORKS_DIR" ]; then
        echo "Copying to Release Frameworks directory: $RELEASE_FRAMEWORKS_DIR"
        cp "$DYLIB_SOURCE" "$RELEASE_FRAMEWORKS_DIR"
        echo "Copied libsoradyne.dylib to Release app bundle Frameworks directory"
    else
        echo "Release Frameworks directory not found: $RELEASE_FRAMEWORKS_DIR"
    fi
    
    echo "Copy script completed successfully"
else
    echo "ERROR: libsoradyne.dylib not found at: $DYLIB_SOURCE"
    echo "Make sure to build the Rust library first with: cargo build --release"
    echo "Current directory: $(pwd)"
    echo "Listing parent directories:"
    ls -la ../../../packages/soradyne_core/target/release/ || echo "Release directory doesn't exist"
fi
