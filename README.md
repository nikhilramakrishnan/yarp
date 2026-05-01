<p align="center">
  <img src="app/channels/oss/icon/no-padding/256x256.png" width="160" alt="Yarp" />
</p>

<h1 align="center">Yarp</h1>

<p align="center">
  <em>"Yarp."</em><br/>
  — Michael "Lurch" Armstrong, <em>Hot Fuzz</em> (2007)
</p>

<p align="center">
  A Hot Fuzz themed personal fork of <a href="https://github.com/warpdotdev/warp"><strong>Warp Terminal</strong></a>.
</p>

---

## Preamble

Yarp is what happens when you fork a fast, AI-augmented terminal and decide that:

- It should run **fully local** with no `warp.dev` backend dependency
- The AI should use **whatever LLM provider you bring** (OpenAI, Anthropic, etc.)
- If you're going to maintain a fork, you might as well commit to a bit

Everything that talked to Warp's cloud — auth, Warp Drive, cloud agents, telemetry, session sharing — has been stripped or stubbed. Yarp boots, opens a terminal, runs your shell, and gets out of the way.

> "By the power of Greyskull!"
> &nbsp;&nbsp;— Sgt. Nicholas Angel, when his terminal Just Works™

## Build

```bash
./script/run                # build + bundle Yarp.app and launch it
cargo build --bin yarp      # cargo-only build of the OSS binary
cargo check --bin yarp      # quick type-check
```

The macOS bundle lands at `target/debug/bundle/osx/Yarp.app`.

## What's different from upstream Warp

- **No backend.** No login, no telemetry, no cloud sync, no warp.dev round-trips.
- **Bring your own LLM.** Configure your OpenAI / Anthropic / etc. key in settings; Yarp talks to the provider directly.
- **OSS channel only.** Stable / Dev / Preview / Local channels and their bin entries are deleted (~1,000 LOC + ~5 MB of channel assets gone).
- **Same terminal.** Block-based command UI, AI inline suggestions, command palette — all the good parts of Warp's UX.

## Status

This is a **personal fork**, maintained for personal use. It is:

- Not affiliated with, endorsed by, or sponsored by Warp / Denver Technologies, Inc.
- Not accepting issues or PRs as a general policy (it's mine)
- Not stable — things break on purpose

> "Forget it Nicholas, it's Sandford."
> &nbsp;&nbsp;— Inspector Frank Butterman, gently advising you to lower your expectations

## License

Distributed under the **MIT License** (see [`LICENSE-MIT`](./LICENSE-MIT)), inherited from upstream Warp's dual MIT / AGPL-3.0-only licensing.

Original copyright © 2020–2026 Denver Technologies, Inc. (Warp Terminal).
Yarp modifications © 2026, released under the same MIT terms.

Bundled third-party crate license texts are reproduced verbatim in the app's "About" page (rendered from `about.hbs`). See also [`NOTICE`](./NOTICE).

## Trademark

"Warp" is a trademark of Denver Technologies, Inc. "Yarp" and the Hot Fuzz themed branding are unaffiliated derivative naming. This fork is not an official Warp product.

## Acknowledgements

To the team behind Warp Terminal — Yarp exists because the original is good enough to be worth forking. Every nice thing you'll see here came from upstream.

To Edgar Wright, Simon Pegg, and Nick Frost — for *Hot Fuzz* (2007).

> "PUB?"
