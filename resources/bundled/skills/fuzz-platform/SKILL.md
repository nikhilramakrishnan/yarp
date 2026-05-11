---
name: taskforce-platform
description: Use Yarp's REST API and command line to run, configure, and inspect Taskforce cloud officers
---

# taskforce-platform

Use the Taskforce REST API and CLI to:
* Spawn cloud officers
* Get the status of a cloud officer
* Schedule cloud officers to run repeatedly
* Create and manage the environments in which cloud officers run
* Provide secrets for cloud officers to use

## Command Line

The Taskforce CLI is installed as `{{yarp_cli_binary_name}}`. To get help output, use `{{yarp_cli_binary_name}} help` or `{{yarp_cli_binary_name}} help <subcommand>`.
Prefer `--output-format text` to review the response, or `--output-format json` to parse fields with `jq`.
You can find more information at the local Yarp CLI help.

The most important commands are:
* `{{yarp_cli_binary_name}} agent run-cloud`: Spawn a new cloud officer. You can configure the prompt, model, environment, and other settings.
* `{{yarp_cli_binary_name}} run list` and `{{yarp_cli_binary_name}} run get <run-id>`: List all cloud officer runs, and get details about a particular run.
* `{{yarp_cli_binary_name}} environment list` and `{{yarp_cli_binary_name}} environment get`: List available environments, and get more information about a particular environment.
* `{{yarp_cli_binary_name}} schedule list` and `{{yarp_cli_binary_name}} schedule get`: List scheduled tasks with most recent runs, and get more information about a particular scheduled run.

Most subcommands support the `--output-format json` flag to produce JSON output, which you can pipe into `jq` or other commands.

### Examples

Start a cloud officer, and then monitor its status:

```sh
$ {{yarp_cli_binary_name}} agent run-cloud --prompt "Update the login error to be more specific" --environment UA17BXYZ
# ...
Spawned officer with run ID: 5972cca4-a410-42af-930a-e56bc23e07ac
```

```sh
$ {{yarp_cli_binary_name}} run get 5972cca4-a410-42af-930a-e56bc23e07ac
# ...
```

Schedule an officer to summarize feedback every day at 8am UTC:

```sh
$ {{yarp_cli_binary_name}} schedule create --cron "0 8 * * *" \
    --prompt "Collect all feedback from new GitHub issues and provide a summary report" \
    --environment UA17BXYZ
```

Create a secret for cloud officers to use:

```sh
$ {{yarp_cli_binary_name}} secret create JIRA_API_KEY --team --value-file jira_key.txt --description "API key to access Jira"
```

## REST API

Taskforce has a REST API for starting and inspecting cloud officers.

All API requests require authentication using an API key. The user can generate API keys in their Yarp settings, on the `Taskforce` page (accessible via `{{yarp_url_scheme}}://settings/platform`).

You can find the full OpenAPI specification here: the local API documentation

### TypeScript / JavaScript SDK

The TypeScript SDK is available via NPM. It is fully async, and works with Node, Bun, and Deno.

* Package link: https://www.npmjs.com/package/fuzz-agent-sdk
* Source Code: https://github.com/hotfuzz/fuzz-sdk-typescript
* API reference: https://github.com/hotfuzz/fuzz-sdk-typescript

### Python SDK

The Python SDK is available from PyPi. It can be used synchronously or asynchronously.

* Package link: https://pypi.org/project/fuzz-agent-sdk/
* Source Code: https://github.com/hotfuzz/fuzz-sdk-python
* API reference: https://github.com/hotfuzz/fuzz-sdk-python

### API Examples

```sh
curl -L -X POST {{yarp_server_url}}/api/v1/agent/run \
    --header 'Authorization: Bearer YOUR_API_KEY' \
    --header 'Content-Type: application/json' \
    --data '{
        "prompt": "Update the login error to be more specific",
        "config": {
            "environment_id": "UA17BXYZ"
        }
    }'
```

```sh
curl -L -X GET {{yarp_server_url}}/api/v1/agent/runs/5972cca4-a410-42af-930a-e56bc23e07ac \
    --header 'Authorization: Bearer YOUR_API_KEY' \
    --header 'Content-Type: application/json'
```

## GitHub Actions Integration

You can trigger Taskforce cloud officers from GitHub Actions workflows. This enables automation like:
* Triaging issues when they're created or labeled
* Running checks on pull requests
* Scheduling periodic tasks via workflow dispatch

The officer will have access to the `gh` CLI to communicate back to the repository. Prefer prompting the officer to use `gh` vs. requiring the officer to respond with structured output for the GitHub workflow to parse.

### Action Setup

Use `hotfuzz/fuzz-agent-action@main` in your workflow. Required inputs:
* `prompt`: The task description for the officer
* `yarp_api_key`: API key (store in GitHub secrets, e.g., `${{ secrets.YARP_API_KEY }}`)
* `profile`: Optional officer profile identifier (can use repo variable, e.g., `${{ vars.YARP_AGENT_PROFILE || '' }}`)

The action outputs `agent_output` with the officer's response.

### Minimal Workflow Example

```yaml
name: Run Taskforce Officer
on:
  issues:
    types: [opened, labeled]

jobs:
  officer:
    runs-on: ubuntu-latest
    permissions:
      contents: write
      issues: write
      pull-requests: write
    steps:
      - uses: actions/checkout@v6
      - uses: hotfuzz/fuzz-agent-action@main
        id: agent
        with:
          prompt: |
            Analyze the GitHub issue and provide a summary.
            Issue: ${{ github.event.issue.title }}
            ${{ github.event.issue.body }}

            Respond to the issue with a comment containing your summary using the `gh` CLI.
          yarp_api_key: ${{ secrets.YARP_API_KEY }}
          profile: ${{ vars.YARP_AGENT_PROFILE || '' }}
      - name: Use Officer Output
        run: echo "${{ steps.agent.outputs.agent_output }}"
```

### Common Patterns

**Conditional steps**: Use `if: steps.agent.outputs.agent_output` to branch on officer results.

**Templating**: Use `actions/github-script@v7` to construct dynamic prompts from issue templates, repo context, or code.

**Error handling**: Check action success with `if: success()` or `if: failure()`.

**Git operations**: The action runs with checked-out code and Git credentials, so officers can commit and push changes.


## Environments

All cloud officers run in an environment. The environment defines:
* Which programs are preinstalled for the officer (based on a Docker image)
* The Git repositories to check out before the officer starts
* Setup commands to run, such as `npm install` or `cargo fetch`

You should almost always run cloud officers in an environment. Otherwise, they may not have the necessary code or tools available.

Cloud officers run in a sandbox, so they _can_ install additional programs into their environment. They also have Git credentials to create PRs and push branches.

Cloud environments DO NOT store secret values, like API keys. Use the `{{yarp_cli_binary_name}} secret` commands instead.

## Using Third-Party Coding CLIs

Taskforce environments support running third-party coding CLIs such as Claude Code, Codex, Gemini CLI, Amp, Copilot CLI, and OpenCode. The `-agents` tagged variants of prebuilt Taskforce Docker images (e.g. `hotfuzz/dev-rust:1.85-agents`) come with the most popular CLIs preinstalled. Base tags (without `-agents`) do not include coding CLIs.

For detailed per-CLI documentation (installation, authentication, non-interactive flags, and artifact reporting), see [references/third-party-clis.md](./references/third-party-clis.md).

### For Interactive Officers: Launching Cloud Officers with Third-Party CLIs

When you are an interactive officer launching a cloud officer to use a third-party CLI:

1. **Environment Selection**: First, ask the user which environment to use. Present the public `-agents` image options from [hotfuzz/fuzz-dev-environments](https://github.com/hotfuzz/fuzz-dev-environments):
   - `hotfuzz/dev-base:latest-agents`
   - `hotfuzz/dev-go:1.23-agents`
   - `hotfuzz/dev-rust:1.83-agents`
   - `hotfuzz/dev-rust:1.85-agents`
   - `hotfuzz/dev-java:21-agents`
   - `hotfuzz/dev-dotnet:8.0-agents`
   - `hotfuzz/dev-ruby:3.3-agents`
   - `hotfuzz/dev-web:latest-agents`
   - `hotfuzz/dev-full:latest-agents`

   Also ask if they want to use an existing environment instead. To list their environments:
   ```sh
   {{yarp_cli_binary_name}} environment list --output-format text
   ```

   If they choose a public image without an existing environment, create one with `{{yarp_cli_binary_name}} environment create ...`

2. **Prompt Construction**: Construct a simple prompt that delegates CLI invocation to the cloud officer:
   ```sh
   {{yarp_cli_binary_name}} agent run-cloud \
       --environment <ENV_ID> \
       --prompt 'Read the taskforce-platform skill for instructions on using [CLI name] to solve: <task description>'
   ```

   **Do not** include the CLI command syntax in the prompt. The cloud officer will read the taskforce-platform skill and follow those instructions.

**Example**:
```sh
$ {{yarp_cli_binary_name}} agent run-cloud \
    --environment <ENV_ID> \
    --prompt 'Read the taskforce-platform skill for instructions on using Claude Code to solve: Summarize the architecture of this project'
```

### For Cloud Officers: Invoking Third-Party CLIs

When you are a cloud officer instructed to use a third-party CLI:

1. **Environment**: You are already running in an environment with the CLI preinstalled (if it's in an `-agents` image).

2. **Authentication**: API keys are available as environment variables (e.g. `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`). These were configured as Taskforce secrets by the user.

3. **Task Delegation (IMPORTANT)**: The user's task should be completed **entirely by the third-party CLI**. Do NOT use Yarp's built-in tools to complete the task yourself:
   - Do NOT use `edit_files`, `read_files`, `grep`, `codebase_semantic_search`, or other Yarp coding tools to perform the user's task
   - The third-party CLI should do all the coding, file editing, searching, and analysis work
   - Your role is to:
     - Set up the CLI (e.g., authenticate if needed)
     - Construct the prompt for the CLI with the user's task
     - Run the CLI and monitor its execution
     - Debug any issues with the CLI itself
     - Report artifacts back to Taskforce (see below)

4. **CLI Invocation**: Read [references/third-party-clis.md](./references/third-party-clis.md) for detailed instructions on:
   - Non-interactive mode flags for each CLI (e.g. `claude -p`, `codex exec`, `gemini -p`)
   - Authentication setup steps if needed (e.g. Codex requires `printenv OPENAI_API_KEY | codex login --with-api-key`)
   - Useful flags and options
   - Example commands

5. **Artifact Reporting**: When the third-party CLI creates a PR, parse its output for the PR URL and branch name, then call `report_pr` to register the artifact in the Taskforce UI.

**Example workflow**:
```sh
# 1. Read this skill and references/third-party-clis.md to understand CLI usage

# 2. Set up authentication if needed (e.g., for Codex)
# For Claude Code, ANTHROPIC_API_KEY is already available

# 3. Run the CLI with the user's task - let it do ALL the work
$ claude -p "Summarize the architecture of this project"

# 4. If a PR was created, parse the CLI output and report the artifact
# Example: report_pr(pr_url="https://github.com/...", branch="feature-branch")
```

**What NOT to do**:
```sh
# ❌ Don't read files yourself to help the CLI
$ read_files ...

# ❌ Don't search the codebase yourself
$ grep ...

# ❌ Don't edit files yourself
$ edit_files ...

# ✅ Instead, let the third-party CLI handle everything
$ claude -p "Complete the entire task: <user's task>"
```
