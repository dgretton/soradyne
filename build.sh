#!/bin/bash
set -e

# Build the Rust library
echo "Building Rust library..."
cargo build --release

# Build the TypeScript bindings
echo "Building TypeScript bindings..."
cd ts
npm install
npm run build
cd ..

echo "Build complete!"
echo "You can now run the examples:"
echo "  cd ts && npm run example:heartrate"
echo "  cd ts && npm run example:chat"
