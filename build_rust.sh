#!/bin/bash

# Build the Rust library for the current platform
echo "Building Soradyne Rust library..."

# Build for the current platform
cargo build --release

# Copy the library to the Flutter app
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS - copy to multiple locations
    cp target/release/libsoradyne.dylib flutter_app/
    echo "Copied libsoradyne.dylib to flutter_app/"
    
    # Also copy to the macOS app bundle location
    mkdir -p flutter_app/macos/Runner/
    cp target/release/libsoradyne.dylib flutter_app/macos/Runner/
    echo "Copied libsoradyne.dylib to flutter_app/macos/Runner/"
    
    # Copy to build directory if it exists
    if [ -d "flutter_app/build/macos/Build/Products/Debug/soradyne_app.app/Contents/MacOS/" ]; then
        cp target/release/libsoradyne.dylib flutter_app/build/macos/Build/Products/Debug/soradyne_app.app/Contents/MacOS/
        echo "Copied libsoradyne.dylib to app bundle"
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
