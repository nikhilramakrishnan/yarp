<p align="center">
  <img src="app/channels/oss/icon/no-padding/256x256.png" width="160" alt="Yarp" />
</p>

<h1 align="center">Yarp</h1>

<p align="center">
  <em>"Yarp."</em><br/>
  — Michael "Lurch" Armstrong, <em>Hot Fuzz</em>
</p>

<p align="center">
  Not everything is as it seems in Sandford.</a>.
</p>

---

## Preamble

I started playing around with Warp because I liked it but ended up thinking it would be funny if we had a terminal that resembled a police station, and every session is a case file. 

> "Forget it Nicholas, it's Sandford."
> &nbsp;&nbsp;— Inspector Frank Butterman, gently advising you to lower your expectations

## Build

```bash
./script/run                # build + bundle Yarp.app and launch it
cargo build --bin yarp      # cargo-only build of the OSS binary
cargo check --bin yarp      # quick type-check
```

The macOS bundle lands at `target/debug/bundle/osx/Yarp.app`.

## Status

This is an unstable personal and unaffiliated fork of Warp but you can use it if you like it.

## Acknowledgements

We love Warp and what they've done for the developer experience. We stand on the shoulders of giants.

To Edgar Wright, Simon Pegg, and Nick Frost — for *Hot Fuzz* (2007).

> "PUB?"
