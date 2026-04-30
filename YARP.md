# YARP.md

## Build

```bash
./script/run
cargo build --bin yarp
cargo check --bin yarp
```

## Local-First Architecture

- User data lives under `~/.yarp/`.
- AI requests go through `OssAiClient`, with provider selection and credentials supplied by environment configuration.
- Object sync does not use the hosted backend; `OssObjectClient` is a local-first stub.
- The Yarp app entry point is the `yarp` binary.
- The `yarp` binary in `src/bin/local.rs` remains the internal Local-channel build.
