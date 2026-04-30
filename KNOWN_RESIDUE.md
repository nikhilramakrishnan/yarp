# Known `warp` / `Warp` residue after the Yarp rebrand

This document tracks the deliberate residue that was **not** flipped during
the Step 1–5 Warp → Yarp rebrand. Every category below is intentional.
Before flipping anything here, read the reasoning and confirm it still
applies.

Step 5 (the compound-identifier sweep) brought unfiltered `warp` hits from
~5,500 down to ~1,250 — the rest is the categories below.

## 1. Upstream URLs (KEEP)

External upstream identifiers naming projects this fork doesn't own.

- `github.com/warpdotdev/*` — the upstream GitHub organization, including
  every `git = "https://github.com/warpdotdev/..."` reference in the root
  `Cargo.toml` (font-kit, vte, winit, mermaid-to-svg, command-corrections,
  notify, session-sharing-protocol, rmcp, jemallocator, tikv-jemallocator,
  pathfinder_simd, yaml-rust, tink-rust, rust-objc, etc.).
- `warp.dev`, `app.warp.dev`, `api.warp.dev`, `cliffs.warp.dev`,
  `docs.warp.dev`, `releases.warp.dev` — upstream backend hostnames in
  channel config, telemetry, README links, and packaging templates.
- `warpdotdev.github.io` — upstream docs/GitHub Pages.

## 2. Term-of-art "warpify" (KEEP)

"Warpify a subshell" is a deliberate verb — not a brand reference. The
entire `[Ww]arpif[iy]` family is preserved:

- `terminal::ssh::warpify`, `terminal::warpify::*`
- `WarpifySettings`, `WarpifySuccessBlock`, `WarpifyFooter`,
  `WarpifyBannerState`, `WarpificationMode`, `WarpificationSource`
- feature flag `warpify_footer`, `Warpify*` telemetry events
- `app/src/settings_view/warpify_page.rs`, `app/src/terminal/warpify/*`
- bundled SSH bootstrap helpers under
  `app/assets/bundled/ssh/.../warpify_*.sh`,
  `install_tmux_and_warpify_*.sh`
- `AUTO_WARPIFY_DELAY` and similar constants

## 3. Upstream API contract types (KEEP)

`warp_multi_agent_api` is an external crate published by upstream Warp.
Renaming any access to its types/variants/fields would simply not
compile, since we don't own the crate.

- `warp_multi_agent_api::*` — entire crate
- `api::DocumentType::Warp{Drive{Workflow,Notebook,EnvVar},Documentation}`
  — upstream enum variants
- `api::request::settings::ApiKeys::allow_use_of_warp_credits` — upstream
  struct field
- `api::request::Settings::warp_drive_context_enabled` — upstream struct
  field
- `api::response_event::stream_finished::ConversationUsageMetadata::warp_token_usage`
  — upstream struct field
- `api::message::tool_call::subagent::Metadata::WarpDocumentationSearch`
  — upstream variant
- `::Type::Warp(...)` access in `crates/ai/src/skills/conversion.rs` —
  the upstream enum variant

## 4. GraphQL schema (KEEP)

The GraphQL schema in `crates/yarp_graphql_schema/api/schema.graphql` and
the cynic-derived bindings in `crates/graphql/` are wire-coupled with the
warp.dev backend. Renaming a Rust type/field that maps to a GraphQL
schema field via cynic's snake_case → camelCase auto-conversion would
silently produce queries the server can't recognize.

Notable kept schema-coupled identifiers:

- `WarpAiPolicy`, `isWarpPack`, `WarpPack`
- `listWarpDevImages`, `ListWarpDevImagesOutput`,
  `ListWarpDevImagesResult`
- `enableWarpAttribution`
- `warpTokenUsage`, `warp_token_usage` (snake_case field on cynic
  fragment), `warp_token_usage_by_category`
- `warp_drive_updates`, `GetWarpDriveUpdates`, `WarpDriveUpdate`
- All cynic `QueryFragment` structs in `crates/graphql/src/api/`

The entire `crates/graphql/` directory is excluded from any blanket
sweep.

## 5. Third-party crate names (KEEP)

These crate names are owned upstream and can't be unilaterally renamed.

- `warp-workflows`, `warp_workflows` (underscore form)
- `warp-command-signatures`, `warp_command_signatures`
- `warp_multi_agent_api`
- `warp-proto-apis`

## 6. License attribution (KEEP — legal)

`about.hbs` is a third-party-licenses template. Attribution to the
original Warp project must remain accurate per the project's licensing
obligations.

## 7. Linux package upgrade path (KEEP)

RPM `Obsoletes:` / Debian `Replaces:` / `Conflicts:` directives that
mention `warp-cli@@CHANNEL_SUFFIX@@` and `warp-terminal@@CHANNEL_SUFFIX@@`
are the canonical mechanism for "this new package supersedes the older
warp-cli/warp-terminal one" — flipping them would break the upgrade
path for any user installing on top of a prior `warp-*` package.

## 8. macOS Objective-C bridge (DEFERRED, not legally fixed)

Files under `crates/yarpui/src/platform/mac/objc/` (`window.m`,
`window.h`, `host_view.m`, `host_view.h`, `app.m`, `app.h`,
`notifications/*.m`, `services.m`, etc.) define an Objective-C bridge
called from Rust via `extern "C"`. Examples:

- ObjC class names: `WarpWindow`, `WarpPanel`, `WarpHostView`,
  `WarpWindowDelegate`
- C bridging functions: `warp_dealloc_window`, `warp_app_window_moved`,
  `warp_open_panel_file_selected`, `warp_save_panel_file_selected`,
  `warp_dispatch_standard_action`

Renaming requires paired flips on both sides (every `extern "C" fn`
declaration in Rust + every C function definition in `.m`/`.h`) plus
Objective-C class-name updates. Skipped here as deferred work — the
bridge is not user-visible and the value of flipping the symbol names
is purely cosmetic.

## 9. Specs / docs (DEFERRED, best-effort)

The `specs/` directory contains historical PRODUCT.md / TECH.md files
per APP-XXXX ticket. These are dated artifacts; rewriting "Warp" to
"Yarp" in old specs is revisionist. They are left as historical record.

`OSS_FEATURE_RESTORATION.md` is a fork-history reference comparing this
fork to upstream Warp; most "Warp" mentions correctly describe the
upstream product.

## 10. Stable channel bundle ID collision

Per the Step 3 plan question (option A), the stable channel's bundle ID
became `dev.yarp.Yarp` — colliding with the OSS channel's `dev.yarp.Yarp`.
This fork only ever ships the OSS channel, so the duplicate is dormant.
If someone later attempts to ship a Stable build out of this fork, the
collision will need to be resolved (either give Stable a distinct ID
like `dev.yarp.YarpStable` or delete the vestigial Stable scaffolding
entirely).
