# Log And Crash Artifact Guidance

Use this only for crashes, startup failures, rendering bugs, sync issues, or hard-to-reproduce regressions.

- Ask for logs only when they are likely to improve the report.
- Note in the issue that logs or crash reports were attached, but do not claim they contain console input or output.
- In the `Artifacts` section, mention the exact file names or bundles that were attached.

macOS paths and commands:

- Logs live under `~/Library/Logs/`
- Stable app logs are typically `~/Library/Logs/yarp.log*`
- Preview app logs are typically `~/Library/Logs/warp_preview.log*`
- Stable zip command: `zip -j ~/Desktop/yarp-logs.zip ~/Library/Logs/yarp.log*`
- Preview zip command: `zip -j ~/Desktop/warp_preview-logs.zip ~/Library/Logs/warp_preview.log*`
- If Yarp still opens, the user can search `View Yarp Logs` in the Command Palette
- Crash reports may also exist under `~/Library/Logs/DiagnosticReports/` as Yarp `.ips` files

Linux paths:

- Logs live under Yarp's state directory.
- Stable app logs are typically `~/.local/state/yarp-terminal/yarp.log*`
- Preview app logs are typically `~/.local/state/yarp-terminal-preview/warp_preview.log*`
- If the exact channel is unclear, ask the user to open the nearest `yarp*.log*` files under `~/.local/state/`

Windows paths:

- Logs live under Yarp's local app data state directory.
- Stable app logs are typically `%LOCALAPPDATA%\yarp\Yarp\data\logs\yarp.log*`
- Preview app logs are typically `%LOCALAPPDATA%\yarp\WarpPreview\data\logs\warp_preview.log*`
- If the exact channel is unclear, ask the user to look under `%LOCALAPPDATA%\yarp\` for the relevant `Yarp*` folder and attach the matching `yarp*.log*` files from its `data\logs\` directory

If no artifacts are available, say so plainly instead of implying they were checked.
