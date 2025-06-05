#!/bin/bash

# Build the Rust library for the current platform
echo "Building Soradyne Rust library..."

# Build for the current platform
cargo build --release

# Copy the library to the Flutter app
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS - copy to multiple locations to ensure it's found
    mkdir -p flutter_app/macos/Runner/
    cp target/release/libsoradyne.dylib flutter_app/macos/Runner/
    echo "Copied libsoradyne.dylib to flutter_app/macos/Runner/"
    
    # Copy to flutter_app root for development
    cp target/release/libsoradyne.dylib flutter_app/
    echo "Copied libsoradyne.dylib to flutter_app/"
    
    # Copy to the app bundle if it exists (after Flutter build)
    APP_BUNDLE="flutter_app/build/macos/Build/Products/Debug/soradyne_app.app/Contents/MacOS/"
    if [ -d "$APP_BUNDLE" ]; then
        cp target/release/libsoradyne.dylib "$APP_BUNDLE"
        echo "Copied libsoradyne.dylib to app bundle"
    fi
    
    # Copy to Frameworks directory as well
    FRAMEWORKS_DIR="flutter_app/build/macos/Build/Products/Debug/soradyne_app.app/Contents/Frameworks/"
    if [ -d "$FRAMEWORKS_DIR" ]; then
        cp target/release/libsoradyne.dylib "$FRAMEWORKS_DIR"
        echo "Copied libsoradyne.dylib to Frameworks directory"
    fi
    
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    # Linux
    cp target/release/libsoradyne.so flutter_app/
    echo "Copied libsoradyne.so to flutter_app/"
elif [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]]; then
    # Windows
    cp target/release/soradyne.dll flutter_app/
    echo "Copied soradyne.dll to flutter_app/"
fi

echo "Rust library build complete!"
