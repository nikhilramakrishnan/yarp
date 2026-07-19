# YARP.md

yarp is a single, static, cross-platform Go binary — a local-first terminal
client with blocks, a ctrl-g overlay (palette/blocks/AI/themes), local-LLM
support, and personal backup connectors. Design record:
`specs/go-rewrite/PRODUCT.md` and `specs/go-rewrite/TECH.md`.

```bash
CGO_ENABLED=0 go build ./cmd/yarp   # build
go test ./...                        # unit tests
./e2e/run.sh                         # four-shell end-to-end matrix
```

## Local-First Architecture

- User data lives under `~/.yarp/` (settings.json, history/, themes/,
  workflows/, memory.jsonl, backups/). No accounts, no telemetry, no cloud.
- AI requests go only to local OpenAI-compatible runtimes (Ollama, LM Studio,
  llama.cpp), configured via settings or `YARP_LLM_BASE_URL`/`YARP_LLM_MODEL`
  — the same env contract the Rust `OssAiClient`/`LocalLlmProvider` used.
  There is no API-key surface.
- Storage is backed up with `yarp backup` to targets the user owns (local
  dir, their own Google Drive, rclone remotes).

## The legacy Rust codebase

The original Rust fork of Warp (app/ + crates/) was removed after the Go
rewrite was verified end-to-end; recover it from git history if ever needed
(it last lived at tag-less commit `b3a515e`'s parent tree). The `specs/`
directory remains as the institutional memory of the work done there.
