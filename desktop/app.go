package main

import (
	"context"
	"encoding/base64"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"time"

	"github.com/nikhilramakrishnan/yarp/desktop/semantic"
	"github.com/nikhilramakrishnan/yarp/internal/blocks"
	"github.com/nikhilramakrishnan/yarp/internal/config"
	"github.com/nikhilramakrishnan/yarp/internal/shellhook"
	"github.com/nikhilramakrishnan/yarp/internal/termio"
	wailsruntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

const desktopBlockOutputLimit = 256 * 1024

type App struct {
	ctxMu sync.RWMutex
	ctx   context.Context

	sessions *SessionManager

	integrationMu sync.Mutex
	integrations  map[string]*terminalIntegration
}

type terminalIntegration struct {
	observer   *semantic.Observer
	store      *blocks.Store
	cleanup    func()
	currentCWD string
}

type SessionInfo struct {
	ID    string `json:"id"`
	Shell string `json:"shell"`
	CWD   string `json:"cwd"`
}

type terminalDataPayload struct {
	SessionID  string `json:"sessionID"`
	Sequence   uint64 `json:"sequence"`
	DataBase64 string `json:"dataBase64"`
}

type terminalExitPayload struct {
	SessionID     string `json:"sessionID"`
	ExitCode      int    `json:"exitCode"`
	Message       string `json:"message"`
	Requested     bool   `json:"requested"`
	FinalSequence uint64 `json:"finalSequence"`
}

type terminalErrorPayload struct {
	SessionID string `json:"sessionID"`
	Operation string `json:"operation"`
	Message   string `json:"message"`
}

type terminalCWDPayload struct {
	SessionID string `json:"sessionID"`
	CWD       string `json:"cwd"`
}

func NewApp() *App {
	app := &App{integrations: make(map[string]*terminalIntegration)}
	app.sessions = NewSessionManager(SessionEventCallbacks{
		OnData:  app.onTerminalData,
		OnExit:  app.onTerminalExit,
		OnError: app.onTerminalError,
	})
	return app
}

func (app *App) startup(ctx context.Context) {
	app.ctxMu.Lock()
	app.ctx = ctx
	app.ctxMu.Unlock()
}

func (app *App) shutdown(context.Context) {
	_ = app.sessions.StopAll()
}

// StartTerminal creates one independent PTY-backed terminal. It never stops
// or redirects another pane's session.
func (app *App) StartTerminal(cols, rows int, requestedCWD string) (SessionInfo, error) {
	cols = clamp(cols, 20, 500)
	rows = clamp(rows, 5, 300)

	shell, plainArgs, err := resolveShell()
	if err != nil {
		return SessionInfo{}, err
	}
	cwd, err := resolveCWD(requestedCWD)
	if err != nil {
		return SessionInfo{}, err
	}
	sessionID, err := newOpaqueSessionID()
	if err != nil {
		return SessionInfo{}, err
	}
	runtimeDir, err := config.Subdir("runtime")
	if err != nil {
		return SessionInfo{}, err
	}
	launch, err := shellhook.Prepare(shellhook.Detect(shell), filepath.Join(runtimeDir, sessionID))
	if err != nil {
		return SessionInfo{}, err
	}
	args := launch.Args
	if shellhook.Detect(shell) == shellhook.Unknown {
		args = plainArgs
	}

	historyDir, err := config.Subdir("history")
	if err != nil {
		launch.Cleanup()
		return SessionInfo{}, err
	}
	store, err := blocks.OpenStore(historyDir, sessionID)
	if err != nil {
		launch.Cleanup()
		return SessionInfo{}, err
	}
	integration := &terminalIntegration{store: store, cleanup: launch.Cleanup, currentCWD: cwd}
	integration.observer = semantic.NewObserver(sessionID, desktopBlockOutputLimit, time.Now, func(update semantic.Update) {
		if update.Terminal() {
			if err := store.Append(&update.Record); err != nil {
				app.emitTerminalError(sessionID, "persist-block", err)
			}
		}
		app.emit("block:update", update)
	})
	app.integrationMu.Lock()
	app.integrations[sessionID] = integration
	app.integrationMu.Unlock()

	env := overlayEnv(os.Environ(), append(shellhook.EnvForSession(sessionID), launch.Env...))
	env = withEnv(env, map[string]string{
		"COLORTERM":    "truecolor",
		"TERM":         "xterm-256color",
		"TERM_PROGRAM": "Yarp",
		"YARP_DESKTOP": "1",
	})
	managed, err := app.sessions.startWithID(sessionID, termio.Command{
		Path: shell,
		Args: args,
		Env:  env,
		Dir:  cwd,
	}, cols, rows)
	if err != nil {
		app.integrationMu.Lock()
		delete(app.integrations, sessionID)
		app.integrationMu.Unlock()
		_ = store.Close()
		launch.Cleanup()
		return SessionInfo{}, err
	}
	return SessionInfo{ID: managed.ID, Shell: managed.Shell, CWD: managed.CWD}, nil
}

func (app *App) WriteTerminal(sessionID, data string) error {
	return app.sessions.Write(sessionID, []byte(data))
}

func (app *App) ResizeTerminal(sessionID string, cols, rows int) error {
	return app.sessions.Resize(sessionID, clamp(cols, 20, 500), clamp(rows, 5, 300))
}

func (app *App) AckTerminalOutput(sessionID string, sequence uint64) error {
	return app.sessions.Ack(sessionID, sequence)
}

func (app *App) StopTerminal(sessionID string) error {
	return app.sessions.Stop(sessionID)
}

// GetRecentBlocks restores real locally persisted command evidence for the
// desktop inspector. The live block:update stream supplies subsequent
// revisions; xterm scrollback is never treated as the history database.
func (app *App) GetRecentBlocks(limit int) ([]semantic.Update, error) {
	historyDir, err := config.Subdir("history")
	if err != nil {
		return nil, err
	}
	recent, err := blocks.LoadRecent(historyDir, clamp(limit, 1, 500))
	if err != nil {
		return nil, err
	}
	updates := make([]semantic.Update, 0, len(recent))
	for _, block := range recent {
		status := semantic.StatusCompleted
		switch {
		case block.ExitCode < 0:
			status = semantic.StatusCancelled
		case block.ExitCode > 0:
			status = semantic.StatusFailed
		}
		updates = append(updates, semantic.Update{
			ID:       blockID(block.Session, block.Seq),
			Revision: 1,
			Status:   status,
			Reason:   "restored",
			Record:   block,
		})
	}
	return updates, nil
}

func (app *App) onTerminalData(event SessionDataEvent) {
	app.integrationMu.Lock()
	integration := app.integrations[event.SessionID]
	app.integrationMu.Unlock()
	if integration != nil {
		integration.observer.Scan(event.Data)
		if cwd := integration.observer.CWD(); cwd != "" && cwd != integration.currentCWD {
			integration.currentCWD = cwd
			app.emit("terminal:cwd", terminalCWDPayload{SessionID: event.SessionID, CWD: cwd})
		}
	}
	app.emit("terminal:data", terminalDataPayload{
		SessionID:  event.SessionID,
		Sequence:   event.Sequence,
		DataBase64: base64.StdEncoding.EncodeToString(event.Data),
	})
}

func (app *App) onTerminalExit(event SessionExitEvent) {
	app.integrationMu.Lock()
	integration := app.integrations[event.SessionID]
	delete(app.integrations, event.SessionID)
	app.integrationMu.Unlock()
	if integration != nil {
		integration.observer.Flush()
		_ = integration.store.Close()
		integration.cleanup()
	}
	message := ""
	if event.Err != nil {
		message = event.Err.Error()
	}
	app.emit("terminal:exit", terminalExitPayload{
		SessionID:     event.SessionID,
		ExitCode:      event.ExitCode,
		Message:       message,
		Requested:     event.Requested,
		FinalSequence: event.FinalDataSequence,
	})
}

func (app *App) onTerminalError(event SessionErrorEvent) {
	app.emitTerminalError(event.SessionID, event.Operation, event.Err)
}

func (app *App) emitTerminalError(sessionID, operation string, err error) {
	if err == nil {
		return
	}
	app.emit("terminal:error", terminalErrorPayload{
		SessionID: sessionID,
		Operation: operation,
		Message:   err.Error(),
	})
}

func (app *App) emit(name string, payload any) {
	app.ctxMu.RLock()
	ctx := app.ctx
	app.ctxMu.RUnlock()
	if ctx != nil {
		wailsruntime.EventsEmit(ctx, name, payload)
	}
}

func resolveCWD(requested string) (string, error) {
	if requested == "" {
		return os.UserHomeDir()
	}
	cwd, err := filepath.Abs(requested)
	if err != nil {
		return "", err
	}
	info, err := os.Stat(cwd)
	if err != nil {
		return "", err
	}
	if !info.IsDir() {
		return "", errors.New("terminal working directory is not a directory")
	}
	return cwd, nil
}

func resolveShell() (string, []string, error) {
	if configured := os.Getenv("YARP_SHELL_PATH"); configured != "" {
		return configured, loginArgs(configured), nil
	}
	if configured := os.Getenv("SHELL"); configured != "" {
		return configured, loginArgs(configured), nil
	}
	if runtime.GOOS == "windows" {
		for _, candidate := range []string{"pwsh.exe", "powershell.exe", "cmd.exe"} {
			if path, err := exec.LookPath(candidate); err == nil {
				return path, loginArgs(path), nil
			}
		}
		return "", nil, errors.New("no supported shell found")
	}
	for _, candidate := range []string{"/bin/zsh", "/bin/bash", "/bin/sh"} {
		if _, err := os.Stat(candidate); err == nil {
			return candidate, loginArgs(candidate), nil
		}
	}
	return "", nil, errors.New("no supported shell found")
}

func loginArgs(shell string) []string {
	name := strings.ToLower(filepath.Base(shell))
	switch name {
	case "pwsh", "pwsh.exe", "powershell", "powershell.exe":
		return []string{"-NoLogo"}
	case "cmd", "cmd.exe":
		return nil
	default:
		return []string{"-l"}
	}
}

func overlayEnv(existing, additions []string) []string {
	replacements := make(map[string]string, len(additions))
	for _, item := range additions {
		key, value, ok := strings.Cut(item, "=")
		if ok {
			replacements[key] = value
		}
	}
	return withEnv(existing, replacements)
}

func withEnv(existing []string, additions map[string]string) []string {
	out := make([]string, 0, len(existing)+len(additions))
	for _, item := range existing {
		key, _, ok := strings.Cut(item, "=")
		if ok {
			if _, replaced := additions[key]; replaced {
				continue
			}
		}
		out = append(out, item)
	}
	for key, value := range additions {
		out = append(out, key+"="+value)
	}
	return out
}

func clamp(value, minimum, maximum int) int {
	if value < minimum {
		return minimum
	}
	if value > maximum {
		return maximum
	}
	return value
}

func blockID(sessionID string, sequence int) string {
	return fmt.Sprintf("%s:%06d", sessionID, sequence)
}
