#!/bin/bash

# Build the Rust library for the current platform
echo "Building Soradyne Rust library..."

# Build for the current platform
cargo build --release

# Copy the library to the Flutter app
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS
    cp target/release/libsoradyne.dylib flutter_app/
    echo "Copied libsoradyne.dylib to flutter_app/"
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
