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

I started playing around with Warp because I liked it but ended up thinking it would be funny if we had a terminal that resembled a police station, and every prompt is a case file. 

> "By the power of Greyskull!"
> &nbsp;&nbsp;— Sgt. Nicholas Angel, when his terminal Just Works™

## Build

```bash
./script/run                # build + bundle Yarp.app and launch it
cargo build --bin yarp      # cargo-only build of the OSS binary
cargo check --bin yarp      # quick type-check
```

The macOS bundle lands at `target/debug/bundle/osx/Yarp.app`.

## Status

This is an unstable personal fork but you can use it if you like it.

- Not affiliated with, endorsed by, or sponsored by Warp / Denver Technologies, Inc.

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

We love Warp and what they've done for the developer experience. We stand on the shoulders of giants.

To Edgar Wright, Simon Pegg, and Nick Frost — for *Hot Fuzz* (2007).

> "PUB?"
