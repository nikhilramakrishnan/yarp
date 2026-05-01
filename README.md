# Yarp

Yarp is a Hot Fuzz themed personal fork of [Warp Terminal](https://github.com/warpdotdev/warp) that runs locally without the warp.dev backend and uses a bring-your-own LLM provider for AI features.

This is an unofficial, personal project. It is not affiliated with, endorsed by, or sponsored by Warp / Denver Technologies, Inc. "Warp" is a trademark of Denver Technologies, Inc.

## Build

```bash
./script/run
cargo build --bin yarp
cargo check --bin yarp
```

## License

Yarp is distributed under the MIT License (see [`LICENSE-MIT`](./LICENSE-MIT)), inherited from upstream Warp's dual MIT / AGPL-3.0-only licensing.

Original copyright © 2020-2026 Denver Technologies, Inc. (Warp Terminal). Yarp modifications are also released under the MIT License.

Bundled third-party crate license texts are reproduced verbatim in the app's "About" page (rendered from `about.hbs`).
