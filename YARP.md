# YARP.md

## Go rewrite (current direction)

yarp is being rewritten as a single, static, cross-platform Go binary in
`go/` — a local-first terminal client with blocks, a ctrl-g overlay
(palette/blocks/AI/themes), local-LLM support, and personal backup
connectors. See `specs/go-rewrite/PRODUCT.md` and `specs/go-rewrite/TECH.md`.

```bash
cd go
CGO_ENABLED=0 go build ./cmd/yarp   # build
go test ./...                        # test
```

## Local-First Architecture

- User data lives under `~/.yarp/` (settings.json, history/, themes/,
  workflows/, memory.jsonl, backups/). No accounts, no telemetry, no cloud.
- AI requests go only to local OpenAI-compatible runtimes (Ollama, LM Studio,
  llama.cpp), configured via settings or `YARP_LLM_BASE_URL`/`YARP_LLM_MODEL`
  — the same env contract the Rust `OssAiClient`/`LocalLlmProvider` used.
  There is no API-key surface.
- Hosted object sync is gone; storage is backed up with `yarp backup` to
  targets the user owns (local dir, their own Google Drive, rclone remotes).

## Legacy Rust build

The original Rust app remains in `app/` + `crates/`:

```bash
./script/run
cargo build --bin yarp
cargo check --bin yarp
```

- The Rust `yarp` binary entry point is `src/bin/local.rs` (internal
  Local-channel build); its `OssObjectClient` is a local-first stub.
