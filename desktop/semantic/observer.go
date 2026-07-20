// Package semantic adapts the existing shell-integration scanner into
// lifecycle updates suitable for the desktop client.
//
// PTY bytes must still be forwarded to the terminal renderer unchanged. An
// Observer only sees a copy of that stream and turns OSC metadata into stable
// block records; it is not a terminal emulator or a persistence layer.
package semantic

import (
	"fmt"
	"strings"
	"time"

	"github.com/nikhilramakrishnan/yarp/internal/blocks"
	"github.com/nikhilramakrishnan/yarp/internal/vt"
)

// Status is the user-visible lifecycle of a semantic command block.
type Status string

const (
	StatusRunning   Status = "running"
	StatusCompleted Status = "completed"
	StatusFailed    Status = "failed"
	StatusCancelled Status = "cancelled"
)

// Update is a versioned snapshot of one block. ID remains stable from the
// first Running update through its terminal update. Record reuses the current
// local history model so callers can persist terminal updates with blocks.Store.
type Update struct {
	ID       string       `json:"id"`
	Revision int          `json:"revision"`
	Status   Status       `json:"status"`
	Reason   string       `json:"reason,omitempty"`
	Record   blocks.Block `json:"record"`
}

// Terminal reports whether the update ends the block lifecycle.
func (u Update) Terminal() bool { return u.Status != StatusRunning }

// Observer converts one session's shell-integration events into block
// lifecycle updates. It is not safe for concurrent use; the owning PTY reader
// should call Scan in stream order.
type Observer struct {
	sessionID string
	now       func() time.Time
	onUpdate  func(Update)
	scanner   *vt.Scanner

	cwd            string
	pendingCommand string
	nextSequence   int
	active         *activeBlock
}

type activeBlock struct {
	id       string
	revision int
	record   blocks.Block
}

// NewObserver creates a semantic observer for one PTY session. textCap is the
// maximum normalized output retained per block; the vt package default is used
// when it is non-positive. A nil clock uses time.Now and a nil callback drops
// updates.
func NewObserver(sessionID string, textCap int, now func() time.Time, onUpdate func(Update)) *Observer {
	if now == nil {
		now = time.Now
	}
	if onUpdate == nil {
		onUpdate = func(Update) {}
	}
	o := &Observer{
		sessionID: sessionID,
		now:       now,
		onUpdate:  onUpdate,
	}
	o.scanner = vt.NewScanner(o.onVTEvent, textCap)
	return o
}

// Scan observes a chunk of PTY output. The caller remains responsible for
// forwarding the original bytes, in order and without modification, to xterm.
func (o *Observer) Scan(p []byte) { o.scanner.Scan(p) }

// CWD returns the most recent working directory reported by OSC 7.
func (o *Observer) CWD() string { return o.cwd }

// Flush terminates an open block honestly when its session ends before OSC
// 133;D arrives. It is safe to call more than once.
func (o *Observer) Flush() {
	if o.active == nil {
		o.pendingCommand = ""
		return
	}
	o.finishActive(StatusCancelled, -1, "session-ended")
}

func (o *Observer) onVTEvent(event vt.Event) {
	switch event.Kind {
	case vt.CommandLine:
		o.pendingCommand = event.Text
		// OSC 633 producers are allowed to report command text after their
		// execution marker. Preserve the stable ID and revise the running block.
		if o.active != nil && o.active.record.Cmd == "" {
			o.active.record.Cmd = event.Text
			o.emitActive(StatusRunning, "")
		}

	case vt.OutputStart:
		if o.active != nil {
			// CommandLine for the new cycle may arrive before OutputStart even
			// when the prior cycle never emitted CommandEnd. Do not let closing
			// the interrupted block discard that exact command text.
			pendingCommand := o.pendingCommand
			o.finishActive(StatusCancelled, -1, "superseded")
			o.pendingCommand = pendingCommand
		}
		o.nextSequence++
		startedAt := o.now()
		o.active = &activeBlock{
			id:       fmt.Sprintf("%s:%06d", o.sessionID, o.nextSequence),
			revision: 0,
			record: blocks.Block{
				Session:   o.sessionID,
				Seq:       o.nextSequence,
				Cmd:       o.pendingCommand,
				CWD:       o.cwd,
				StartedAt: startedAt,
				ExitCode:  -1,
			},
		}
		o.pendingCommand = ""
		o.scanner.StartCapture()
		o.emitActive(StatusRunning, "")

	case vt.CommandEnd:
		if o.active != nil {
			status := StatusCompleted
			if event.ExitCode > 0 {
				status = StatusFailed
			}
			o.finishActive(status, event.ExitCode, "")
			return
		}

		// PowerShell cannot emit a preexec boundary. Its hook reports the exact
		// command and exit code together at the following prompt, so represent
		// that honestly as an output-less, zero-duration terminal block.
		if o.pendingCommand != "" {
			o.nextSequence++
			now := o.now()
			status := StatusCompleted
			if event.ExitCode > 0 {
				status = StatusFailed
			}
			u := Update{
				ID:       fmt.Sprintf("%s:%06d", o.sessionID, o.nextSequence),
				Revision: 1,
				Status:   status,
				Record: blocks.Block{
					Session:   o.sessionID,
					Seq:       o.nextSequence,
					Cmd:       o.pendingCommand,
					CWD:       o.cwd,
					StartedAt: now,
					EndedAt:   now,
					ExitCode:  event.ExitCode,
				},
			}
			o.pendingCommand = ""
			o.onUpdate(u)
		}

	case vt.PromptStart:
		if o.active != nil {
			o.finishActive(StatusCancelled, -1, "prompt-restarted")
		}
		// A prompt begins a new input cycle. Do not let command metadata from a
		// malformed, incomplete cycle label a later command.
		o.pendingCommand = ""

	case vt.CWDChanged:
		if event.Text != "" {
			o.cwd = event.Text
		}
	}
}

func (o *Observer) finishActive(status Status, exitCode int, reason string) {
	output, truncated := o.scanner.StopCapture()
	o.active.record.EndedAt = o.now()
	o.active.record.ExitCode = exitCode
	o.active.record.Output = cleanCaptured(output)
	o.active.record.Truncated = truncated
	o.emitActive(status, reason)
	o.active = nil
	o.pendingCommand = ""
}

func (o *Observer) emitActive(status Status, reason string) {
	o.active.revision++
	o.onUpdate(Update{
		ID:       o.active.id,
		Revision: o.active.revision,
		Status:   status,
		Reason:   reason,
		Record:   o.active.record,
	})
}

// cleanCaptured keeps the current history behavior while the durable desktop
// store is still being designed: strip per-line padding, trailing blank lines,
// and zsh's PROMPT_SP end-of-line marker.
func cleanCaptured(output string) string {
	lines := strings.Split(output, "\n")
	for i := range lines {
		lines[i] = strings.TrimRight(lines[i], " ")
	}
	n := len(lines)
	for n > 0 && lines[n-1] == "" {
		n--
	}
	if n > 0 && (lines[n-1] == "%" || lines[n-1] == "#") {
		n--
		for n > 0 && lines[n-1] == "" {
			n--
		}
	}
	return strings.Join(lines[:n], "\n")
}
