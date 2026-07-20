# Yarp desktop rewrite

## Summary

Yarp is rebuilt as a standalone, local-first desktop environment for terminal work and inspectable software agents. The rewrite keeps Yarp's command → block → evidence → agent loop and Sandford identity, while deliberately dropping inherited hosted-product baggage and any feature that does not strengthen that loop.

## Problem

The previous rewrite replaced Yarp with a wrapper around another terminal. That removed the product's defining spatial workspace, semantic blocks, serious command editor, integrated agents, and visual identity. This rewrite changes the implementation without replacing the product.

## Goals

- Ship a real desktop application with its own windows, terminal panes, navigation, editor, and persisted workspace.
- Make commands and agent actions durable, inspectable objects rather than transient lines of text.
- Preserve local ownership, hackability, and the recognizable Sandford product language.
- Improve or remove inherited features freely when doing so makes Yarp more coherent, reliable, or understandable.
- Deliver and validate useful vertical slices early instead of hiding the product behind a long infrastructure rewrite.

## Non-goals

- Pixel-for-pixel reproduction of the Rust client or exhaustive Warp feature parity.
- Accounts, billing, teams, referrals, hosted object sync, telemetry, corporate release channels, or inherited dogfood/admin surfaces.
- Pretending that a host-terminal wrapper is a desktop client.
- Supporting every operating system before the macOS experience is coherent and dependable.
- Recreating unfinished legacy abstractions solely because they existed before.

## Figma

Figma: none provided. The pre-deletion Yarp client, its canonical portrait assets, Sandford/Greater Good themes, and recorded product flows are the visual and behavioral references.

## Behavior

### Product identity and first launch

1. Yarp launches from Finder or the operating system's application launcher as its own desktop application. It does not open Terminal.app, require an existing terminal window, or present a CLI wrapper as the primary product.

2. First launch opens a useful local shell immediately. An account, network connection, cloud entitlement, API key, or onboarding ceremony is never required to reach a working terminal.

3. The first usable frame is recognizably Yarp: the canonical portrait icon, Sandford navy/cream/red palette, restrained station/case language, native window controls, workspace navigation, and terminal surface appear as one coherent product.

4. Sandford language reinforces the model without obscuring common actions. Familiar controls retain familiar labels where themed language would make the interface harder to understand; contextual subtitles may add terms such as beat, case file, playbook, evidence, or dispatch.

5. The application remains useful when every optional model provider, agent CLI, workflow, and network connection is absent. Missing optional capabilities produce clear empty states and setup paths rather than blocking launch.

### Windows, workspaces, tabs, and panes

6. A workspace is a persistent desk containing one or more tabs. Each tab contains a pane tree that can represent terminal, agent, editor, evidence, settings, or other first-party surfaces.

7. A user can create, rename, reorder, duplicate, and close tabs. Closing a tab with a running process or unsaved work requires an intentional confirmation unless the user has explicitly disabled that warning.

8. A user can split a pane horizontally or vertically, resize split boundaries, move focus between panes entirely by keyboard, and convert or move panes without losing their underlying session state.

9. Maximizing a pane is reversible and does not destroy the surrounding pane layout. The user can return to the exact prior layout.

10. Recently closed tabs and panes can be reopened with their meaningful local state when recovery is possible. A process that cannot be resurrected is represented honestly as ended rather than silently replaced by a new shell.

11. Window, workspace, tab, pane, focus, and working-directory state survive a normal application restart. Restoration failures are isolated to the affected item and do not prevent the rest of the workspace from opening.

12. Saved layouts capture spatial structure and launch intent without embedding secrets. A saved layout can be reviewed as a local file and reopened later.

### Terminal fidelity

13. Every terminal pane owns a real pseudoterminal and behaves like a serious terminal: interactive shells, full-screen applications, alternate screen, colors, Unicode, emoji, IME, mouse protocols, selection, clipboard, bracketed paste, resize, SSH, tmux, vim, and pagers work without Yarp stealing their input.

14. Application shortcuts do not consume ordinary terminal control sequences while focus is inside a terminal. Global actions use platform-appropriate application modifiers or an explicit command surface, and users can remap them.

15. Terminal bytes are never modified merely to make Yarp's UI easier to implement. Yarp may observe shell-integration metadata, but the running program and renderer receive the original stream in order.

16. High-volume output remains responsive and lossless within documented resource limits. If Yarp must apply a limit, it exposes the limit, marks the affected record, and never presents truncated output as complete.

17. A crashed or exited process leaves an inspectable ended pane with its exit state and captured evidence. The user can restart it deliberately or close it.

### Semantic command blocks

18. A shell command becomes a semantic block containing, when available: prompt, exact command, output, working directory, repository and branch, start/end time, duration, exit status, environment/session identity, truncation state, and related agent evidence.

19. Blocks appear in the terminal's natural reading flow. Their chrome is quiet during ordinary use and reveals status, selection, and actions without making command output feel like a collection of unrelated modal screens.

20. A running block is visibly distinct from a completed, failed, cancelled, restored, or truncated block. Status remains understandable without relying on color alone.

21. Users can select one or many blocks and copy commands, copy output, rerun or edit commands, bookmark evidence, filter output, search within output, and save a useful command as a playbook.

22. Block actions never execute a command merely because a search or palette result was selected. Potentially destructive execution always requires the same deliberate submit action as manually entered input.

23. Block records persist locally and remain searchable across sessions. Corrupt or partially written records are isolated and surfaced; they do not make unrelated history unavailable.

24. Rich content such as links, images, structured diffs, file references, tables, and agent actions may render as native block content while preserving an accessible plain-text or source representation for copy and export.

### Command editor and navigation

25. Shell input is a real multiline editor anchored to the active terminal flow, not a thin text box layered over a host terminal. It supports selection, undo/redo, syntax-aware movement, multiline paste, history, and predictable submit/cancel behavior.

26. The editor can surface shell-aware completions, autosuggestions, corrections, history, playbooks, slash commands, and explicit context attachments without silently rewriting the user's input.

27. Search and the command palette provide one fast keyboard surface for tabs, panes, files, settings, blocks, playbooks, projects, and actions. Results state what will happen before activation.

28. Search over terminal history can match command, output, directory, repository, branch, exit status, time, bookmark, and related case/agent metadata. Filters compose and can be cleared without losing the underlying query.

29. Focus, selection, and editor contents survive opening and closing palettes, inspectors, or sidebars. Dismissing transient UI returns the user to the exact prior interaction state.

### Playbooks, projects, and local customization

30. Reusable commands are playbooks stored in user- or project-owned files. Project playbooks can live with a repository, be reviewed in version control, and declare arguments without executing hidden setup work.

31. Playbook arguments support descriptions, defaults, choices, validation, and shell eligibility. Filling a playbook produces inspectable editor text before execution.

32. Settings, themes, keybindings, personas, providers, and layouts have documented local representations. Changes made through the UI and changes made directly to supported files converge on the same state.

33. Yarp ships Sandford dark and Greater Good light themes using the canonical palette. Themes affect the entire product coherently, including terminal colors, block states, editor, agents, selection, and accessibility states.

34. User data has a clear home under `~/.yarp`; project-scoped configuration has a clear `.yarp` location. Yarp explains what it persists and offers direct ways to inspect, export, prune, and delete it.

### Agents and case files

35. Agents operate inside the same case file as terminal work. Their prompts, reasoning summaries, tool requests, shell actions, file reads, searches, edits, diffs, approvals, child tasks, failures, and results appear as inspectable blocks rather than an opaque chat transcript.

36. No agent executes shell commands, edits files, accesses secrets, or expands its authority without the permission policy currently visible to the user. Approval choices clearly state their scope and duration.

37. Agent work can cite terminal blocks, files, selections, repositories, and previous evidence explicitly. The user can inspect and remove attached context before sending.

38. Local agent CLIs and model providers are first-class adapters when installed. Yarp does not require one proprietary hosted provider, and local-first does not artificially prohibit user-configured remote providers.

39. Multiple officers or child agents may work concurrently, but Yarp presents ownership, progress, handoffs, conflicts, and the final synthesized result clearly. Parallelism never hides the underlying actions.

40. A case file persists locally and can be reopened with its evidence graph intact. Missing tools or providers degrade individual actions, not the ability to inspect prior work.

### Privacy, safety, accessibility, and recovery

41. Command output, prompts, files, agent context, and credentials are treated as sensitive local data. Yarp does not transmit them unless the user invokes a configured provider or destination whose scope is visible.

42. Secrets and provider grants are not copied into ordinary unencrypted history exports or backups by default. Any export that includes sensitive configuration says so before creation.

43. Yarp supports a private mode that pauses durable command/output capture while preserving terminal functionality. The active capture state is always visible.

44. All primary terminal, tab, pane, block, editor, search, approval, and settings flows are keyboard accessible. Focus indicators, screen-reader labels, reduced motion, scalable UI text, and non-color status cues are supported.

45. Application crashes and forced restarts do not corrupt the workspace database or silently lose already committed blocks. Recovery explains what was restored, what ended, and what could not be recovered.

46. Updates and migrations are reversible within a documented compatibility window. Yarp creates a recoverable backup before changing durable formats and never overwrites an unknown newer format.

47. A feature is complete only when its visible states, empty/error paths, keyboard behavior, persistence behavior, and recovery behavior work in the packaged desktop application—not merely in an isolated component or CLI test.
