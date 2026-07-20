//go:build !windows

package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/nikhilramakrishnan/yarp/internal/blocks"
)

func TestDesktopSessionCapturesARealSemanticBlock(t *testing.T) {
	if _, err := os.Stat("/bin/zsh"); err != nil {
		t.Skip("zsh is not installed")
	}
	yarpHome := t.TempDir()
	cleanZdot := t.TempDir()
	workdir := t.TempDir()
	t.Setenv("YARP_HOME", yarpHome)
	t.Setenv("YARP_SHELL_PATH", "/bin/zsh")
	t.Setenv("ZDOTDIR", cleanZdot)

	app := NewApp()
	info, err := app.StartTerminal(100, 30, workdir)
	if err != nil {
		t.Fatal(err)
	}
	defer app.StopTerminal(info.ID) //nolint:errcheck

	stopAcks := make(chan struct{})
	defer close(stopAcks)
	go func() {
		ticker := time.NewTicker(5 * time.Millisecond)
		defer ticker.Stop()
		for {
			select {
			case <-ticker.C:
				session, sessionErr := app.sessions.session(info.ID)
				if sessionErr != nil {
					continue
				}
				session.mu.Lock()
				sequence := session.sentSequence
				session.mu.Unlock()
				_ = app.AckTerminalOutput(info.ID, sequence)
			case <-stopAcks:
				return
			}
		}
	}()

	if err := app.WriteTerminal(info.ID, "printf 'desktop-e2e-output\\n'\n"); err != nil {
		t.Fatal(err)
	}
	historyDir := filepath.Join(yarpHome, "history")
	var captured blocks.Block
	deadline := time.Now().Add(4 * time.Second)
	for time.Now().Before(deadline) {
		recent, loadErr := blocks.LoadRecent(historyDir, 4)
		if loadErr == nil {
			for _, block := range recent {
				if strings.Contains(block.Cmd, "desktop-e2e-output") {
					captured = block
					break
				}
			}
		}
		if captured.Cmd != "" {
			break
		}
		time.Sleep(20 * time.Millisecond)
	}
	if captured.Cmd == "" {
		t.Fatal("real zsh command was not persisted as a semantic block")
	}
	resolvedWorkdir, err := filepath.EvalSymlinks(workdir)
	if err != nil {
		t.Fatal(err)
	}
	resolvedCapturedCWD, err := filepath.EvalSymlinks(captured.CWD)
	if err != nil {
		t.Fatal(err)
	}
	if resolvedCapturedCWD != resolvedWorkdir || captured.ExitCode != 0 || !strings.Contains(captured.Output, "desktop-e2e-output") {
		t.Fatalf("captured block = %+v", captured)
	}
}

func TestResolveCWDDefaultsToUserHome(t *testing.T) {
	got, err := resolveCWD("")
	if err != nil {
		t.Fatal(err)
	}
	home, err := os.UserHomeDir()
	if err != nil {
		t.Fatal(err)
	}
	if got != home {
		t.Fatalf("default cwd = %q, want home %q", got, home)
	}
}

func TestOverlayEnvReplacesExistingKeys(t *testing.T) {
	got := overlayEnv([]string{"A=old", "KEEP=yes"}, []string{"A=new", "B=added"})
	joined := strings.Join(got, "\n")
	for _, want := range []string{"A=new", "B=added", "KEEP=yes"} {
		if !strings.Contains(joined, want) {
			t.Fatalf("overlay %q does not contain %q", got, want)
		}
	}
	if strings.Contains(joined, "A=old") {
		t.Fatalf("overlay retained replaced value: %q", got)
	}
}
