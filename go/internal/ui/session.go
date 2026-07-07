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
	settings   *config.Settings
	homeDir    string
	paletteKey byte

	pty     termio.Pty
	scanner *vt.Scanner
	store   *blocks.Store

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
	ov         *overlay
	ptyBuf     bytes.Buffer // child output withheld while the overlay is up
	ptyDropped bool         // buffer overflowed; discarding until close

	// stdin decoding state
	stdinSeq  seqTracker
	stdinRest []byte           // partial escape sequence / rune between chunks
	escCh     <-chan time.Time // fires to resolve a pending lone ESC as a keypress

	// ai plumbing (see ai.go)
	aiAgent  *agent.Agent
	aiGen    int // generation counter; stale deltas/results are dropped
	aiDelta  chan aiDelta
	aiDone   chan aiResult
	aiCancel context.CancelFunc

	backupCh chan string // async "Back up now" results
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
		settings:   settings,
		homeDir:    home,
		paletteKey: settings.PaletteByte(),
		pty:        p,
		store:      store,
		out:        os.Stdout,
		cols:       cols,
		rows:       rows,
		aiDelta:    make(chan aiDelta, 64),
		aiDone:     make(chan aiResult, 1),
		backupCh:   make(chan string, 1),
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
				s.bufferUnderOverlay(chunk)
			} else {
				s.out.Write(chunk)
			}

		case chunk, ok := <-stdinCh:
			if !ok {
				return s.finish(0)
			}
			s.handleStdin(chunk)

		case d := <-s.aiDelta:
			if d.gen == s.aiGen {
				s.aiAppendDelta(d.text)
				// Batch: drain whatever else already arrived before the
				// (comparatively expensive) repaint.
			drain:
				for {
					select {
					case d := <-s.aiDelta:
						if d.gen == s.aiGen {
							s.aiAppendDelta(d.text)
						}
					default:
						break drain
					}
				}
				s.repaint()
			}

		case res := <-s.aiDone:
			if res.gen == s.aiGen {
				s.aiFinish(res)
				s.repaint()
			}

		case msg := <-s.backupCh:
			if s.ov != nil {
				s.ov.status = msg
				s.repaint()
			}

		case <-s.escCh:
			s.resolvePendingEsc()

		case code := <-exitCh:
			return s.finish(code)

		case <-resize.C:
			s.pollResize()
		}
	}
}

// handleStdin routes one chunk of user input.
func (s *Session) handleStdin(chunk []byte) {
	if s.ov == nil {
		s.passthrough(chunk)
		return
	}
	s.overlayInput(chunk)
}

// passthrough forwards input to the pty, watching for the palette hotkey.
// The hotkey only fires on a byte in ground state: BEL and ESC occur inside
// terminal reply sequences and bracketed pastes, which must pass through
// untouched.
func (s *Session) passthrough(chunk []byte) {
	start := 0
	for i := 0; i < len(chunk); i++ {
		b := chunk[i]
		ground := s.stdinSeq.ground()
		s.stdinSeq.feed(b)
		if b == s.paletteKey && ground {
			if i > start {
				s.pty.Write(chunk[start:i])
			}
			s.openOverlay()
			s.overlayInput(chunk[i+1:])
			return
		}
	}
	if start < len(chunk) {
		s.pty.Write(chunk[start:])
	}
}

// overlayInput decodes keys for the overlay. If the overlay closes mid-batch
// (Esc followed by more typing in one read), the remaining raw bytes belong
// to the shell and are forwarded verbatim — no lossy re-encoding.
func (s *Session) overlayInput(chunk []byte) {
	buf := s.stdinRest
	s.stdinRest = nil
	buf = append(buf, chunk...)
	i := 0
	for i < len(buf) {
		if s.ov == nil {
			s.passthrough(buf[i:])
			s.armEscTimer()
			return
		}
		k, n, need := DecodeOne(buf[i:])
		if need {
			s.stdinRest = append([]byte(nil), buf[i:]...)
			break
		}
		i += n
		s.handleOverlayKey(k)
	}
	if s.ov != nil {
		s.repaint()
	}
	s.armEscTimer()
}

// armEscTimer starts a short timer when the pending input is a lone ESC:
// if nothing follows, it was an Esc keypress; if bytes arrive first, it was
// the start of a split escape sequence.
func (s *Session) armEscTimer() {
	if s.ov != nil && len(s.stdinRest) == 1 && s.stdinRest[0] == 0x1b {
		s.escCh = time.After(60 * time.Millisecond)
	} else {
		s.escCh = nil
	}
}

func (s *Session) resolvePendingEsc() {
	s.escCh = nil
	if s.ov == nil || len(s.stdinRest) != 1 || s.stdinRest[0] != 0x1b {
		return
	}
	s.stdinRest = nil
	s.handleOverlayKey(Key{Kind: KeyEsc})
	if s.ov != nil {
		s.repaint()
	}
}

// bufferUnderOverlay withholds child output while the overlay covers the
// screen. The buffer is capped: replaying an unbounded log dump on close is
// as useless as storing it, so past the cap yarp discards (the scanner has
// already seen every byte, so block recording is unaffected) and says so
// when the overlay closes.
const ptyBufCap = 4 << 20

func (s *Session) bufferUnderOverlay(chunk []byte) {
	if s.ptyDropped {
		return
	}
	s.ptyBuf.Write(chunk)
	if s.ptyBuf.Len() > ptyBufCap {
		s.ptyBuf.Reset()
		s.ptyDropped = true
	}
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
			// Integrations without a preexec hook (pwsh) report the command
			// and exit code only, with no OutputStart: record an output-less
			// block rather than dropping the command.
			if s.pendCmd != "" {
				now := time.Now()
				s.record(blocks.Block{
					Cmd: s.pendCmd, CWD: s.cwd,
					StartedAt: now, EndedAt: now,
					ExitCode: ev.ExitCode,
				})
				s.pendCmd = ""
			}
			return
		}
		out, trunc := s.scanner.StopCapture()
		s.record(blocks.Block{
			Cmd:       s.pendCmd,
			CWD:       s.cwd,
			StartedAt: s.pendStart,
			EndedAt:   time.Now(),
			ExitCode:  ev.ExitCode,
			Output:    strings.TrimRight(out, "\n"),
			Truncated: trunc,
		})
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

func (s *Session) record(b blocks.Block) {
	if b.Cmd == "" && b.Output == "" {
		return
	}
	_ = s.store.Append(&b)
	s.recent = append(s.recent, b)
	if len(s.recent) > 64 {
		s.recent = s.recent[1:]
	}
}

func (s *Session) flushPendingBlock() {
	if !s.inCommand {
		return
	}
	out, trunc := s.scanner.StopCapture()
	s.record(blocks.Block{
		Cmd: s.pendCmd, CWD: s.cwd,
		StartedAt: s.pendStart, EndedAt: time.Now(),
		ExitCode: -1, Output: strings.TrimRight(out, "\n"), Truncated: trunc,
	})
}

// readerChan pumps an io.Reader into a channel of right-sized owned chunks.
func readerChan(r io.Reader) <-chan []byte {
	ch := make(chan []byte, 8)
	go func() {
		defer close(ch)
		buf := make([]byte, 32*1024)
		for {
			n, err := r.Read(buf)
			if n > 0 {
				ch <- append([]byte(nil), buf[:n]...)
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
