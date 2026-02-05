# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Soradyne is a proof-of-concept protocol for secure, peer-to-peer Self-Data Flows with CRDT-based synchronization. The monorepo also contains Giantt, a task dependency management system being ported from Python to Dart to use Soradyne as its sync backend.

**Reference materials** (do not edit):
- `giantt-original/` - Symlink to original Python CLI (external, for reference only)
- `docs/port_reference/` - Python source files being ported (`giantt_core.py`, `giantt_cli.py`)
- `giantt-design-notes/` - Feature specs, notation docs, CLI manual

## Monorepo Structure

```
apps/
  giantt/              # Flutter app for Giantt (task management UI)
  soradyne_demo/       # Flutter demo app showcasing Soradyne capabilities
packages/
  giantt_core/         # Dart: Core giantt logic (parser, graph, storage, validation)
  soradyne_core/       # Rust: Core Soradyne library (identity, storage, network, album, video)
  soradyne_flutter/    # Flutter plugin wrapping soradyne_core via FFI
  ai_chat_flutter/     # Flutter package for AI chat with action calling
```

## Build Commands

### Rust (soradyne_core)
```bash
# Build
cargo build --release
# or via melos:
melos run build:rust

# Run examples
cargo run --example block_storage_demo
cargo run --example heartrate_demo
```

### Flutter/Dart
```bash
# Bootstrap all packages (run from repo root)
melos bootstrap

# Run tests
melos run test:flutter
# or for a specific package:
cd packages/giantt_core && dart test
cd apps/giantt && flutter test

# Run analysis
melos run analyze

# Run Giantt app
cd apps/giantt && flutter run

# Run Giantt CLI
dart run giantt_core:giantt
```

### Full Build
```bash
./build.sh  # Builds Rust library and TypeScript bindings
```

## Architecture Notes

### Soradyne Core (Rust)
- `storage/` - Data dissolution (erasure coding via reed-solomon) and device identity
- `flow/` - Real-time data streaming
- `identity/` - Cryptographic identity management
- `network/` - P2P networking (warp-based HTTP, designed for BLE expansion)
- `album/` - Photo album SDO implementation
- `video/` - Video frame extraction (optional ffmpeg feature)
- `ffi/` - Foreign function interface for Flutter bindings

### Giantt Core (Dart)
- `models/` - GianttItem, Relation, TimeConstraint, Duration, Status, Priority
- `parser/` - Parses `.giantt` text files into item graphs
- `graph/` - Dependency graph with cycle detection (inline in `giantt_graph.dart`)
- `storage/` - FileRepository, DualFileManager (include/occlude system), atomic writes, backups
- `validation/` - GraphDoctor for finding issues in graphs
- `logging/` - Log entry tracking with occlusion support
- `commands/` - CLI commands extending `CliCommand<T>` base class

**Key design notes:**
- Items support multiple `timeConstraints` (List), not singular
- Include/occlude system: active items in `include/`, archived in `occlude/`
- Relations are bidirectional (auto-created in graph operations)

### Key Concepts
- **SelfDataFlow**: The core Rust abstraction (`flow/mod.rs`) for user-owned data streams that sync peer-to-peer. Generic over any `T: Send + Sync + Clone`. Examples: `HeartrateFlow`, `SelfDataFlow<RobotJointState>`
- **Data Dissolution**: Splitting data across multiple devices using erasure coding for security/resilience
- **Data Crystallization**: Recombining dissolved data when devices reunite
- **Giantt Items**: Task nodes with relations (REQUIRES, ANYOF, BLOCKS, etc.), time constraints, and status tracking
