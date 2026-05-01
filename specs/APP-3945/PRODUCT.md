# APP-3945: Channel-aware Yarp home watching Product Spec

## Summary
Yarp should hot-reload the current channel's Yarp-managed files without reacting to unrelated files under `.yarp*/worktrees`. This includes continuing to reload `settings.toml` correctly on platforms where settings live under `config_local_dir()` instead of `data_dir()`.

## Problem
Yarp currently relies on filesystem watching for several user-visible behaviors: reloading themes, workflows, launch configs, tab configs, Yarp home MCP config, Yarp home skills, and public settings from `settings.toml`. The watcher surface is easy to regress because Yarp-managed files are split across different directories depending on platform and channel.

The specific failure modes this work addresses are:
- changes under `.yarp*/worktrees` can produce false-positive updates for Yarp home watchers
- a watcher rooted only at `data_dir()` can miss `settings.toml` on Linux and Windows, where `config_local_dir()` differs from `data_dir()`
- fresh installs or hermetic test environments can fail to watch missing directories unless Yarp prepares those roots before registering the watcher

## Goals
- Watch the current channel's Yarp-managed directories through a single Yarp-specific watcher model.
- Ignore filesystem activity under `.yarp*/worktrees` so worktree contents do not trigger Yarp home reload behavior.
- Continue reloading `settings.toml` when it changes on every supported platform, including platforms where settings live outside `data_dir()`.
- Preserve existing hot-reload behavior for themes, workflows, launch configs, tab configs, Yarp home MCP config, and Yarp home skills.

## Non-goals
- Changing where any Yarp-managed file is stored.
- Changing the semantics of settings parsing, settings migration, or settings validation.
- Adding new user-facing UI for watcher state or diagnostics.
- Expanding watch coverage to arbitrary files outside Yarp-managed directories.
- Changing the generic repository watcher APIs used for project repositories.

## Figma / design references
Figma: none provided

## User Experience

### Watch scope
- Yarp watches the current channel's Yarp-owned filesystem roots through a single singleton watcher.
- `data_dir()` remains the source of truth for channel-scoped Yarp home content such as themes, workflows, launch configs, tab configs, MCP config, and skills.
- `config_local_dir()` is also watched when it is a different directory from `data_dir()`.
- When both path helpers resolve to the same directory, Yarp behaves as before and does not create duplicate logical coverage.

### Settings hot reload
- When `settings.toml` changes, Yarp reloads public settings from disk and applies the new values to in-memory settings models.
- This behavior must work whether `settings.toml` lives in the same directory as the rest of Yarp home files or in a separate config directory.
- Creating, modifying, renaming into place, or deleting `settings.toml` must continue to flow through the existing `YarpConfigUpdateEvent::Settings` path.

### Worktree exclusion
- Files under `.yarp`, `.yarp-dev`, `.yarp-local`, or equivalent channel-scoped Yarp home directories that are nested inside `worktrees/` must not trigger Yarp home reload behavior.
- Editing files inside a cloned repository stored under `.yarp*/worktrees/...` must not cause Yarp to reload themes, workflows, tab configs, MCP config, skills, or settings.

### Channel awareness
- Yarp only reacts to files under the active channel's directories.
- A stable or dev install should not reload in response to files written into another channel's Yarp home.

### Fresh-install and test-environment behavior
- If a watched Yarp-owned root directory does not exist yet, Yarp should create it during startup/setup before registering the watcher.
- Missing directories must not silently disable hot reload for the rest of the session.

### No regressions for existing consumers
- Editing a theme file in Yarp home still updates the available theme set.
- Editing workflows, launch configs, or tab configs in Yarp home still refreshes those objects.
- Editing Yarp home MCP config still updates file-based MCP servers.
- Editing Yarp home skills still refreshes Yarp-provided skills.

## Success Criteria
- `settings.toml` hot reload works on macOS, Linux, and Windows.
- Worktree activity under `.yarp*/worktrees` no longer triggers Yarp home reloads.
- Themes, workflows, launch configs, tab configs, Yarp MCP config, and Yarp skills continue to hot reload from the current channel's Yarp home.
- Yarp prepares missing watch roots before attempting to register watchers.
- The watcher architecture remains centralized behind a Yarp-specific singleton instead of reintroducing separate ad hoc watchers for individual consumers.

## Validation
- Unit-test the watcher filtering behavior so updates outside the kept prefix are excluded and cross-boundary moves are handled correctly.
- Run the end-to-end settings hot-reload integration test that edits `settings.toml` multiple times and verifies the in-memory settings model changes after each write.
- Manually or through existing automated coverage, verify that editing Yarp home themes, skills, and MCP config still produces the expected reload behavior.
- Confirm via code review that only `data_dir()` receives the `worktrees` exclusion and `config_local_dir()` remains unfiltered.

## Open questions
- None currently.
