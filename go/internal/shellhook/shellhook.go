// Package shellhook injects block-boundary shell integration into the child
// shell, replacing the Rust bootstrap scripts in app/assets/bundled/bootstrap.
//
// Where the old scripts spoke a private JSON-over-DCS protocol (OSC 9277/9278),
// these hooks emit the open OSC 133 prompt-marking standard plus one private
// sequence (OSC 6973;cmd;<base64>) carrying the exact command line. Any
// terminal that already understands OSC 133 keeps working, and yarp blocks
// work over SSH by sourcing the same snippet remotely.
package shellhook

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// Kind identifies a supported shell family.
type Kind string

const (
	Bash    Kind = "bash"
	Zsh     Kind = "zsh"
	Fish    Kind = "fish"
	Pwsh    Kind = "pwsh"
	Unknown Kind = ""
)

// Detect classifies a shell binary path.
func Detect(shellPath string) Kind {
	base := strings.TrimSuffix(filepath.Base(shellPath), ".exe")
	switch base {
	case "bash":
		return Bash
	case "zsh":
		return Zsh
	case "fish":
		return Fish
	case "pwsh", "powershell":
		return Pwsh
	}
	return Unknown
}

// Snippet returns the integration source for a shell kind, for `yarp init`.
func Snippet(k Kind) (string, bool) {
	switch k {
	case Bash:
		return bashHook, true
	case Zsh:
		return zshHook, true
	case Fish:
		return fishHook, true
	case Pwsh:
		return pwshHook, true
	}
	return "", false
}

// Launch describes how to start the shell with hooks injected.
type Launch struct {
	Args    []string // extra argv for the shell
	Env     []string // extra environment entries
	Cleanup func()   // removes temp files; call after the session ends
}

// Prepare writes hook files under runtimeDir and returns launch arguments for
// the shell. Unknown shells launch plainly: yarp still works as a passthrough
// terminal, just without block boundaries.
func Prepare(k Kind, runtimeDir string) (Launch, error) {
	noop := Launch{Cleanup: func() {}}
	if err := os.MkdirAll(runtimeDir, 0o700); err != nil {
		return noop, err
	}
	rm := func(paths ...string) func() {
		return func() {
			for _, p := range paths {
				os.RemoveAll(p)
			}
		}
	}
	switch k {
	case Bash:
		rc := filepath.Join(runtimeDir, "bash-rc.sh")
		body := bashSourceUserRC + bashHook
		if err := os.WriteFile(rc, []byte(body), 0o600); err != nil {
			return noop, err
		}
		return Launch{Args: []string{"--rcfile", rc}, Cleanup: rm(rc)}, nil
	case Zsh:
		// A ZDOTDIR shim: our .zshrc restores the user's ZDOTDIR, sources
		// their zshrc, then installs the hooks.
		zdot := filepath.Join(runtimeDir, "zdot")
		if err := os.MkdirAll(zdot, 0o700); err != nil {
			return noop, err
		}
		body := zshSourceUserRC + zshHook
		if err := os.WriteFile(filepath.Join(zdot, ".zshrc"), []byte(body), 0o600); err != nil {
			return noop, err
		}
		userZdot := os.Getenv("ZDOTDIR")
		env := []string{"ZDOTDIR=" + zdot, "YARP_USER_ZDOTDIR=" + userZdot}
		return Launch{Env: env, Cleanup: rm(zdot)}, nil
	case Fish:
		hook := filepath.Join(runtimeDir, "hook.fish")
		if err := os.WriteFile(hook, []byte(fishHook), 0o600); err != nil {
			return noop, err
		}
		return Launch{Args: []string{"-C", "source " + hook}, Cleanup: rm(hook)}, nil
	case Pwsh:
		hook := filepath.Join(runtimeDir, "hook.ps1")
		if err := os.WriteFile(hook, []byte(pwshHook), 0o600); err != nil {
			return noop, err
		}
		return Launch{Args: []string{"-NoExit", "-Command", ". '" + hook + "'"}, Cleanup: rm(hook)}, nil
	}
	return noop, nil
}

const bashSourceUserRC = `# yarp bootstrap: load the user's own config first.
if [ -f "$HOME/.bashrc" ]; then . "$HOME/.bashrc"; fi
`

const bashHook = `# yarp shell integration (OSC 133 + OSC 6973). Safe to source twice.
if [ -z "$YARP_HOOKED" ]; then
  YARP_HOOKED=1
  __yarp_osc() { printf '\033]%s\007' "$1"; }
  __yarp_cwd() { __yarp_osc "7;file://$HOSTNAME$PWD"; }
  __yarp_preexec() {
    [ -n "$YARP_IN_CMD" ] && return
    case "$BASH_COMMAND" in __yarp_*) return ;; esac
    YARP_IN_CMD=1
    local b64
    b64=$(printf '%s' "$BASH_COMMAND" | base64 2>/dev/null | tr -d '\n')
    [ -n "$b64" ] && __yarp_osc "6973;cmd;$b64"
    __yarp_osc "133;C"
  }
  __yarp_precmd() {
    local exit=$?
    if [ -n "$YARP_IN_CMD" ]; then
      __yarp_osc "133;D;$exit"
      unset YARP_IN_CMD
    fi
    __yarp_cwd
    __yarp_osc "133;A"
  }
  PROMPT_COMMAND="__yarp_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
  trap '__yarp_preexec' DEBUG
fi
`

const zshSourceUserRC = `# yarp bootstrap: restore the user's ZDOTDIR and load their config.
if [[ -n "$YARP_USER_ZDOTDIR" ]]; then
  ZDOTDIR="$YARP_USER_ZDOTDIR"
else
  ZDOTDIR="$HOME"
fi
unset YARP_USER_ZDOTDIR
[[ -f "$ZDOTDIR/.zshrc" ]] && source "$ZDOTDIR/.zshrc"
`

const zshHook = `# yarp shell integration (OSC 133 + OSC 6973).
if [[ -z "$YARP_HOOKED" ]]; then
  YARP_HOOKED=1
  __yarp_osc() { printf '\033]%s\007' "$1"; }
  __yarp_preexec() {
    local b64
    b64=$(printf '%s' "$1" | base64 2>/dev/null | tr -d '\n')
    [[ -n "$b64" ]] && __yarp_osc "6973;cmd;$b64"
    __yarp_osc "133;C"
    YARP_IN_CMD=1
  }
  __yarp_precmd() {
    local exit=$?
    if [[ -n "$YARP_IN_CMD" ]]; then
      __yarp_osc "133;D;$exit"
      unset YARP_IN_CMD
    fi
    __yarp_osc "7;file://$HOST$PWD"
    __yarp_osc "133;A"
  }
  autoload -Uz add-zsh-hook
  add-zsh-hook preexec __yarp_preexec
  add-zsh-hook precmd __yarp_precmd
fi
`

const fishHook = `# yarp shell integration (OSC 133 + OSC 6973).
if not set -q YARP_HOOKED
  set -g YARP_HOOKED 1
  function __yarp_osc
    printf '\033]%s\007' $argv[1]
  end
  function __yarp_preexec --on-event fish_preexec
    set -l b64 (printf '%s' "$argv[1]" | base64 2>/dev/null | tr -d '\n')
    test -n "$b64"; and __yarp_osc "6973;cmd;$b64"
    __yarp_osc "133;C"
    set -g YARP_IN_CMD 1
  end
  function __yarp_postexec --on-event fish_postexec
    __yarp_osc "133;D;$status"
    set -e YARP_IN_CMD
  end
  function __yarp_prompt --on-event fish_prompt
    __yarp_osc "7;file://$hostname$PWD"
    __yarp_osc "133;A"
  end
end
`

const pwshHook = `# yarp shell integration for PowerShell.
# preexec has no native hook; the command line is reported from history at
# the next prompt, which is enough to delimit and label blocks.
if (-not $env:YARP_HOOKED) {
  $env:YARP_HOOKED = "1"
  $global:__yarpFirstPrompt = $true
  $global:__yarpOldPrompt = $function:prompt
  function global:prompt {
    $exit = if ($global:LASTEXITCODE -ne $null) { $global:LASTEXITCODE } elseif ($?) { 0 } else { 1 }
    if (-not $global:__yarpFirstPrompt) {
      $last = Get-History -Count 1
      if ($last) {
        $b64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($last.CommandLine))
        [Console]::Write("$([char]27)]6973;cmd;$b64$([char]7)")
      }
      [Console]::Write("$([char]27)]133;D;$exit$([char]7)")
    }
    $global:__yarpFirstPrompt = $false
    $cwd = (Get-Location).Path -replace '\\', '/'
    [Console]::Write("$([char]27)]7;file://$env:COMPUTERNAME/$cwd$([char]7)")
    [Console]::Write("$([char]27)]133;A$([char]7)")
    & $global:__yarpOldPrompt
  }
}
`

// EnvForSession returns the environment additions every hooked shell gets.
func EnvForSession(sessionID string) []string {
	return []string{
		"YARP=1",
		"YARP_SESSION_ID=" + sessionID,
		"TERM_PROGRAM=yarp",
	}
}

// InitUsage is the help text for `yarp init`.
func InitUsage() string {
	return fmt.Sprintf("usage: yarp init <%s|%s|%s|%s>\n\nPrints the shell integration snippet. yarp injects it automatically for\nshells it launches; use this to hook shells yarp did not start (e.g. over ssh).",
		Bash, Zsh, Fish, Pwsh)
}
