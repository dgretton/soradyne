#!/bin/bash

# Build the Rust library for the current platform
echo "Building Soradyne Rust library..."

# Build for the current platform
cargo build --release

# Copy the library to the Flutter app (paths relative to packages/soradyne_core/)
APP_DIR="../../apps/soradyne_demo"

if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS - copy to multiple locations to ensure it's found
    mkdir -p "$APP_DIR/macos/Runner/"
    cp target/release/libsoradyne.dylib "$APP_DIR/macos/Runner/"
    echo "Copied libsoradyne.dylib to $APP_DIR/macos/Runner/"

    # Copy to app root for development
    cp target/release/libsoradyne.dylib "$APP_DIR/"
    echo "Copied libsoradyne.dylib to $APP_DIR/"

    # Copy to the app bundle if it exists (after Flutter build)
    APP_BUNDLE="$APP_DIR/build/macos/Build/Products/Debug/soradyne_app.app/Contents/MacOS/"
    if [ -d "$APP_BUNDLE" ]; then
        cp target/release/libsoradyne.dylib "$APP_BUNDLE"
        echo "Copied libsoradyne.dylib to app bundle"
    fi

    # Copy to Frameworks directory as well
    FRAMEWORKS_DIR="$APP_DIR/build/macos/Build/Products/Debug/soradyne_app.app/Contents/Frameworks/"
    if [ -d "$FRAMEWORKS_DIR" ]; then
        cp target/release/libsoradyne.dylib "$FRAMEWORKS_DIR"
        echo "Copied libsoradyne.dylib to Frameworks directory"
    fi

elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    # Linux
    cp target/release/libsoradyne.so "$APP_DIR/"
    echo "Copied libsoradyne.so to $APP_DIR/"
elif [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]]; then
    # Windows
    cp target/release/soradyne.dll "$APP_DIR/"
    echo "Copied soradyne.dll to $APP_DIR/"
fi

echo "Rust library build complete!"
