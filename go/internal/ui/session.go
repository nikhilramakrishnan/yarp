package ui

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"time"

	"golang.org/x/term"

	"github.com/nikhilramakrishnan/yarp/go/internal/agent"
	"github.com/nikhilramakrishnan/yarp/go/internal/blocks"
	"github.com/nikhilramakrishnan/yarp/go/internal/config"
	"github.com/nikhilramakrishnan/yarp/go/internal/shellhook"
	"github.com/nikhilramakrishnan/yarp/go/internal/termio"
	"github.com/nikhilramakrishnan/yarp/go/internal/themes"
	"github.com/nikhilramakrishnan/yarp/go/internal/vt"
)

// Session is one interactive yarp run: a shell on a pty, tee'd through the
// vt scanner, with the overlay available on a hotkey. One goroutine reads
// the pty, one reads stdin, and the main loop owns all state — no locks.
type Session struct {
	settings *config.Settings
	homeDir  string

	pty     termio.Pty
	scanner *vt.Scanner
	store   *blocks.Store
	cleanup func()

	out  *os.File
	cols int
	rows int

	// block recording state
	cwd       string
	pendCmd   string
	pendStart time.Time
	inCommand bool
	recent    []blocks.Block // completed this session, newest last

	// overlay state
	ov     *overlay
	ptyBuf bytes.Buffer // child output withheld while the overlay is up
	dirty  bool         // buffered output exists → nudge a repaint on close

	// ai plumbing (see ai.go)
	aiAgent  *agent.Agent
	aiDelta  chan string
	aiDone   chan aiResult
	aiCancel context.CancelFunc

	stdinRest []byte // partial escape sequence between chunks
}

// Run starts the shell and blocks until it exits, returning its exit code.
func Run(settings *config.Settings) (int, error) {
	if !term.IsTerminal(int(os.Stdin.Fd())) {
		return 1, fmt.Errorf("yarp is an interactive terminal client; run it from a terminal")
	}
	home, err := config.Dir()
	if err != nil {
		return 1, err
	}
	historyDir, err := config.Subdir("history")
	if err != nil {
		return 1, err
	}
	runtimeDir := filepath.Join(home, "runtime")

	sessionID := blocks.SessionID(time.Now(), os.Getpid())
	store, err := blocks.OpenStore(historyDir, sessionID)
	if err != nil {
		return 1, err
	}
	defer store.Close()

	shellPath := resolveShell(settings)
	kind := shellhook.Detect(shellPath)
	launch, err := shellhook.Prepare(kind, filepath.Join(runtimeDir, sessionID))
	if err != nil {
		return 1, err
	}
	defer launch.Cleanup()

	cols, rows, err := term.GetSize(int(os.Stdin.Fd()))
	if err != nil {
		cols, rows = 80, 24
	}
	env := append(os.Environ(), shellhook.EnvForSession(sessionID)...)
	env = append(env, launch.Env...)
	p, err := termio.Start(termio.Command{Path: shellPath, Args: launch.Args, Env: env}, cols, rows)
	if err != nil {
		return 1, fmt.Errorf("starting %s: %w", shellPath, err)
	}
	defer p.Close()

	oldState, err := term.MakeRaw(int(os.Stdin.Fd()))
	if err != nil {
		return 1, err
	}
	defer term.Restore(int(os.Stdin.Fd()), oldState)

	s := &Session{
		settings: settings,
		homeDir:  home,
		pty:      p,
		store:    store,
		out:      os.Stdout,
		cols:     cols,
		rows:     rows,
		aiDelta:  make(chan string, 64),
		aiDone:   make(chan aiResult, 1),
	}
	s.scanner = vt.NewScanner(s.onVTEvent, settings.BlockOutputLimitKB*1024)
	s.applyThemeOnStartup()

	exit := s.loop()

	// Retention: prune old sessions on the way out, when it can't slow
	// anything down.
	_, _ = blocks.Prune(historyDir, time.Duration(settings.HistoryDays)*24*time.Hour, time.Now())
	return exit, nil
}

func (s *Session) applyThemeOnStartup() {
	if s.settings.Theme == "" {
		return
	}
	themesDir := filepath.Join(s.homeDir, "themes")
	if t, ok := themes.Find(themesDir, s.settings.Theme); ok {
		s.out.WriteString(t.OSC())
	}
}

// loop is the single owner of session state.
func (s *Session) loop() int {
	ptyCh := readerChan(s.pty)
	stdinCh := readerChan(os.Stdin)
	exitCh := make(chan int, 1)
	go func() {
		code, _ := s.pty.Wait()
		exitCh <- code
	}()
	resize := time.NewTicker(400 * time.Millisecond)
	defer resize.Stop()

	paletteByte := s.settings.PaletteByte()
	for {
		select {
		case chunk, ok := <-ptyCh:
			if !ok {
				// EOF usually races the exit notification; wait for it.
				select {
				case code := <-exitCh:
					return s.finish(code)
				case <-time.After(2 * time.Second):
					return s.finish(0)
				}
			}
			s.scanner.Scan(chunk)
			if s.ov != nil {
				s.ptyBuf.Write(chunk)
				s.dirty = true
			} else {
				s.out.Write(chunk)
			}

		case chunk, ok := <-stdinCh:
			if !ok {
				return s.finish(0)
			}
			s.handleStdin(chunk, paletteByte)

		case delta := <-s.aiDelta:
			s.aiAppendDelta(delta)
			s.repaint()

		case res := <-s.aiDone:
			s.aiFinish(res)
			s.repaint()

		case code := <-exitCh:
			return s.finish(code)

		case <-resize.C:
			s.pollResize()
		}
	}
}

func (s *Session) handleStdin(chunk []byte, paletteByte byte) {
	if s.ov == nil {
		// Passthrough — except the hotkey that opens the overlay.
		if i := bytes.IndexByte(chunk, paletteByte); i >= 0 {
			if i > 0 {
				s.pty.Write(chunk[:i])
			}
			s.openOverlay()
			// Anything typed after the hotkey in the same chunk feeds the
			// overlay.
			chunk = chunk[i+1:]
			if len(chunk) == 0 {
				return
			}
		} else {
			s.pty.Write(chunk)
			return
		}
	}
	buf := append(s.stdinRest, chunk...)
	keys, rest := DecodeKeys(buf)
	s.stdinRest = rest
	for _, k := range keys {
		if s.ov == nil {
			// The overlay closed mid-batch (e.g. Esc then more typing):
			// remaining keys belong to the shell.
			s.pty.Write(keyBytes(k))
			continue
		}
		s.handleOverlayKey(k)
	}
	if s.ov != nil {
		s.repaint()
	}
}

// keyBytes re-encodes a decoded key for the pty (used only for the tail of
// a chunk that closed the overlay).
func keyBytes(k Key) []byte {
	switch k.Kind {
	case KeyRune:
		return []byte(string(k.Rune))
	case KeyEnter:
		return []byte{'\r'}
	case KeyBackspace:
		return []byte{0x7f}
	case KeyTab:
		return []byte{'\t'}
	case KeyCtrl:
		return []byte{k.Ctrl}
	case KeyEsc:
		return []byte{0x1b}
	}
	return nil
}

func (s *Session) pollResize() {
	cols, rows, err := term.GetSize(int(os.Stdin.Fd()))
	if err != nil || (cols == s.cols && rows == s.rows) {
		return
	}
	s.cols, s.rows = cols, rows
	s.pty.Resize(cols, rows)
	if s.ov != nil {
		s.repaint()
	}
}

func (s *Session) finish(code int) int {
	if s.ov != nil {
		s.closeOverlay()
	}
	s.flushPendingBlock()
	s.out.WriteString(sgrReset)
	return code
}

// onVTEvent receives shell-integration events synchronously from the scanner.
func (s *Session) onVTEvent(ev vt.Event) {
	switch ev.Kind {
	case vt.CommandLine:
		s.pendCmd = ev.Text
	case vt.OutputStart:
		s.pendStart = time.Now()
		s.inCommand = true
		s.scanner.StartCapture()
	case vt.CommandEnd:
		if !s.inCommand {
			return
		}
		out, trunc := s.scanner.StopCapture()
		out = strings.TrimRight(out, "\n")
		b := blocks.Block{
			Cmd:       s.pendCmd,
			CWD:       s.cwd,
			StartedAt: s.pendStart,
			EndedAt:   time.Now(),
			ExitCode:  ev.ExitCode,
			Output:    out,
			Truncated: trunc,
		}
		if b.Cmd != "" || len(out) > 0 {
			_ = s.store.Append(&b)
			s.recent = append(s.recent, b)
			if len(s.recent) > 64 {
				s.recent = s.recent[1:]
			}
		}
		s.pendCmd = ""
		s.inCommand = false
	case vt.PromptStart:
		// A prompt with a command still open means the shell never reported
		// D (e.g. hooks half-installed); drop the dangling capture.
		if s.inCommand {
			s.scanner.StopCapture()
			s.inCommand = false
			s.pendCmd = ""
		}
	case vt.CWDChanged:
		if ev.Text != "" {
			s.cwd = ev.Text
		}
	}
}

func (s *Session) flushPendingBlock() {
	if !s.inCommand {
		return
	}
	out, trunc := s.scanner.StopCapture()
	if s.pendCmd == "" && len(out) == 0 {
		return
	}
	_ = s.store.Append(&blocks.Block{
		Cmd: s.pendCmd, CWD: s.cwd,
		StartedAt: s.pendStart, EndedAt: time.Now(),
		ExitCode: -1, Output: out, Truncated: trunc,
	})
}

// readerChan pumps an io.Reader into a channel of owned chunks.
func readerChan(r io.Reader) <-chan []byte {
	ch := make(chan []byte, 8)
	go func() {
		defer close(ch)
		for {
			buf := make([]byte, 32*1024)
			n, err := r.Read(buf)
			if n > 0 {
				ch <- buf[:n]
			}
			if err != nil {
				return
			}
		}
	}()
	return ch
}

func resolveShell(settings *config.Settings) string {
	if settings.Shell != "" {
		return settings.Shell
	}
	if sh := os.Getenv("YARP_SHELL_PATH"); sh != "" {
		return sh // kept from the Rust app: devs test alternate shells this way
	}
	return defaultShell()
}
