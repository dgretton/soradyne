# Soradyne Protocol

A protocol for secure, peer-to-peer shared self-data objects with a focus on privacy and user control.

Made with Aider Chat Bot backed by Claude Sonnet 4 hosted on OpenRouter.

## What is Soradyne?

Soradyne is a protocol that enables secure, peer-to-peer sharing of self-data objects (SDOs). It's designed to give users full control over their personal data, allowing them to share it with others on their terms.

Key features:

- **Data Sovereignty**: You own and control your data, not a third-party service or company.
- **Peer-to-Peer**: Direct sharing between devices without requiring centralized servers.
- **Data Dissolution**: Split your data across multiple devices for enhanced security and resilience.
- **Data Crystallization**: Recombine dissolved data when needed.
- **Real-Time & Eventually Consistent SDOs**: Support for different synchronization models based on use case.
- **Encrypted & Private**: All data is encrypted by default, and privacy is built into the core design.

## Use Cases

Soradyne is designed to support various use cases, including:

- **Heart Rate Monitoring**: Share real-time heart rate data with healthcare providers or family members.
- **Chat Conversations**: Private, end-to-end encrypted messaging between individuals or groups.
- **Photo Albums**: Share photos with friends and family without uploading them to a cloud service.
- **File Sharing**: Securely share files with others directly, with no size limits or usage tracking.
- **Collaborative Robotics**: Share real-time robot joint positions and forces for remote collaboration.

## Quick Start

### Prerequisites

- Rust (latest stable)
- Node.js (v14 or higher)
- npm or yarn

### Building

```bash
# Clone the repository
git clone https://github.com/your-username/soradyne.git
cd soradyne

# Build the library
./build.sh
```

### Running Examples

```bash
# Heart rate example
cd ts && npm run example:heartrate

# Chat example
cd ts && npm run example:chat
```

## Architecture

Soradyne is built with a layered architecture:

1. **Core Layer**: Identity management, cryptography, and transport protocols.
2. **SDO Layer**: Self-Data Object definitions and implementations.
3. **Storage Layer**: Data dissolution and crystallization mechanisms.
4. **Application Layer**: Examples and applications built on top of Soradyne.

## Project Status

Soradyne is currently in the early development phase. It's not yet ready for production use, but we welcome contributions and feedback!

## License

This project is licensed under the MIT License - see the LICENSE file for details.
