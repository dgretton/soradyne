#!/bin/bash
# Copy the Rust library to the app bundle after build
APP_BUNDLE="build/macos/Build/Products/Release/soradyne_app.app/Contents/MacOS/"
FRAMEWORKS_DIR="build/macos/Build/Products/Release/soradyne_app.app/Contents/Frameworks/"

if [ -f "libsoradyne.dylib" ]; then
    if [ -d "$APP_BUNDLE" ]; then
        cp libsoradyne.dylib "$APP_BUNDLE"
        echo "Post-build: Copied libsoradyne.dylib to app bundle MacOS directory"
    fi
    
    if [ -d "$FRAMEWORKS_DIR" ]; then
        cp libsoradyne.dylib "$FRAMEWORKS_DIR"
        echo "Post-build: Copied libsoradyne.dylib to app bundle Frameworks directory"
    fi
else
    echo "libsoradyne.dylib not found in current directory"
fi
