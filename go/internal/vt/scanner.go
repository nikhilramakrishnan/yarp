// Package vt implements a streaming scanner for the child shell's output.
//
// yarp does not re-implement a terminal emulator: the user's native terminal
// renders everything. The scanner tees the byte stream through unchanged and,
// on the way past, extracts the shell-integration events that delimit blocks
// (OSC 133), report the working directory (OSC 7), carry the command line
// (OSC 633;E and yarp's private OSC 6973), and set the window title. It also
// accumulates a plain-text (ANSI-stripped) copy of command output so blocks
// can be persisted and searched.
package vt

import (
	"encoding/base64"
	"net/url"
	"strconv"
	"strings"
)

// EventKind identifies a shell-integration event.
type EventKind int

const (
	// PromptStart: OSC 133;A — the shell is about to draw a prompt.
	PromptStart EventKind = iota
	// CommandStart: OSC 133;B — the user finished the prompt; input begins.
	CommandStart
	// OutputStart: OSC 133;C — the command was accepted; output follows.
	OutputStart
	// CommandEnd: OSC 133;D;<exit> — the command finished.
	CommandEnd
	// CWDChanged: OSC 7;file://host/path — working directory report.
	CWDChanged
	// CommandLine: OSC 633;E;<cmd> or OSC 6973;cmd;<base64> — the command text.
	CommandLine
	// TitleChanged: OSC 0/2;<title>.
	TitleChanged
)

// Event is one extracted shell-integration event.
type Event struct {
	Kind     EventKind
	ExitCode int    // CommandEnd only; -1 when the shell didn't report one
	Text     string // CWD path, command line, or title
}

const (
	esc = 0x1b
	bel = 0x07
	st  = 0x9c // 8-bit string terminator
)

type state int

const (
	stGround  state = iota
	stEsc           // saw ESC
	stCSI           // ESC [ ... final byte
	stOSC           // ESC ] ... BEL / ST
	stOSCEsc        // inside OSC, saw ESC (possible ESC \ terminator)
	stStr           // DCS/SOS/PM/APC ... ST
	stStrEsc        // inside string, saw ESC
	stCharset       // ESC ( / ) / * / + — one byte follows
)

// Scanner is a push-based byte scanner. It is not safe for concurrent use.
type Scanner struct {
	state   state
	oscBuf  []byte
	onEvent func(Event)

	// text receives the ANSI-stripped printable stream while capture is on.
	text     strings.Builder
	capture  bool
	textCap  int
	overflow bool
	lastCR   bool
}

// NewScanner returns a scanner that invokes onEvent for each extracted event.
// textCap bounds the accumulated plain text in bytes; 0 means 256 KiB.
func NewScanner(onEvent func(Event), textCap int) *Scanner {
	if textCap <= 0 {
		textCap = 256 * 1024
	}
	return &Scanner{onEvent: onEvent, textCap: textCap}
}

// StartCapture begins accumulating plain text (typically at OutputStart).
func (s *Scanner) StartCapture() {
	s.text.Reset()
	s.overflow = false
	s.capture = true
}

// StopCapture ends accumulation and returns the captured text and whether it
// was truncated at the cap.
func (s *Scanner) StopCapture() (string, bool) {
	s.capture = false
	return s.text.String(), s.overflow
}

// Scan consumes a chunk of child output. The caller forwards the same chunk
// to the real terminal unchanged; Scan never modifies it.
func (s *Scanner) Scan(p []byte) {
	for _, b := range p {
		switch s.state {
		case stGround:
			switch {
			case b == esc:
				s.state = stEsc
			case b == st:
				// stray 8-bit terminator; ignore
			case b == 0x9d: // 8-bit OSC introducer
				s.oscBuf = s.oscBuf[:0]
				s.state = stOSC
			case s.capture:
				s.captureByte(b)
			}
		case stEsc:
			switch b {
			case '[':
				s.state = stCSI
			case ']':
				s.oscBuf = s.oscBuf[:0]
				s.state = stOSC
			case 'P', 'X', '^', '_': // DCS, SOS, PM, APC
				s.state = stStr
			case '(', ')', '*', '+':
				s.state = stCharset
			default:
				s.state = stGround
			}
		case stCharset:
			s.state = stGround
		case stCSI:
			// parameter/intermediate bytes are 0x20..0x3f; final is 0x40..0x7e
			if b >= 0x40 && b <= 0x7e {
				s.state = stGround
			}
		case stOSC:
			switch b {
			case bel:
				s.finishOSC()
			case st:
				s.finishOSC()
			case esc:
				s.state = stOSCEsc
			default:
				if len(s.oscBuf) < 1<<20 {
					s.oscBuf = append(s.oscBuf, b)
				}
			}
		case stOSCEsc:
			if b == '\\' {
				s.finishOSC()
			} else {
				// Not a terminator; treat the ESC as data loss and resync.
				s.oscBuf = s.oscBuf[:0]
				s.state = stGround
			}
		case stStr:
			if b == esc {
				s.state = stStrEsc
			} else if b == st {
				s.state = stGround
			}
		case stStrEsc:
			switch b {
			case '\\':
				s.state = stGround
			case esc:
				// consecutive ESC: stay armed for a following backslash
			default:
				s.state = stStr
			}
		}
	}
}

func (s *Scanner) captureByte(b byte) {
	if s.text.Len() >= s.textCap {
		s.overflow = true
		return
	}
	switch {
	case b == '\r':
		// Progress-bar style rewrites: fold CR into a newline; a following
		// LF (CRLF) is then swallowed so line endings don't double up.
		s.pendingCR()
		s.lastCR = true
		return
	case b == '\n':
		if !s.lastCR {
			s.text.WriteByte('\n')
		}
	case b == '\t':
		s.text.WriteByte('\t')
	case b >= 0x20:
		s.text.WriteByte(b)
	}
	s.lastCR = false
}

// pendingCR handles CR by emitting a newline only if the last written byte
// wasn't already one, so CRLF doesn't double up.
func (s *Scanner) pendingCR() {
	str := s.text.String()
	if len(str) > 0 && str[len(str)-1] != '\n' {
		s.text.WriteByte('\n')
	}
}

func (s *Scanner) finishOSC() {
	body := string(s.oscBuf)
	s.oscBuf = s.oscBuf[:0]
	s.state = stGround
	if ev, ok := ParseOSC(body); ok {
		s.onEvent(ev)
	}
}

// ParseOSC interprets one OSC payload (without introducer/terminator).
func ParseOSC(body string) (Event, bool) {
	code, rest, _ := strings.Cut(body, ";")
	switch code {
	case "133":
		kind, arg, _ := strings.Cut(rest, ";")
		switch kind {
		case "A":
			return Event{Kind: PromptStart}, true
		case "B":
			return Event{Kind: CommandStart}, true
		case "C":
			return Event{Kind: OutputStart}, true
		case "D":
			exit := -1
			if n, err := strconv.Atoi(strings.TrimSpace(arg)); err == nil {
				exit = n
			}
			return Event{Kind: CommandEnd, ExitCode: exit}, true
		}
	case "633":
		// VS Code shell integration; E carries the command line with
		// backslash escaping.
		kind, arg, _ := strings.Cut(rest, ";")
		if kind == "E" {
			// arg may be followed by ;<nonce>; take the first field only.
			cmd, _, _ := strings.Cut(arg, ";")
			return Event{Kind: CommandLine, Text: unescape633(cmd)}, true
		}
	case "6973":
		// yarp's private channel: 6973;cmd;<base64 command line>
		kind, arg, _ := strings.Cut(rest, ";")
		if kind == "cmd" {
			if raw, err := base64.StdEncoding.DecodeString(strings.TrimSpace(arg)); err == nil {
				return Event{Kind: CommandLine, Text: string(raw)}, true
			}
		}
	case "7":
		return Event{Kind: CWDChanged, Text: parseFileURL(rest)}, true
	case "0", "2":
		return Event{Kind: TitleChanged, Text: rest}, true
	}
	return Event{}, false
}

// unescape633 reverses VS Code's OSC 633 escaping: `\\` and `\xHH`.
func unescape633(s string) string {
	if !strings.Contains(s, "\\") {
		return s
	}
	var out strings.Builder
	for i := 0; i < len(s); i++ {
		if s[i] != '\\' || i+1 >= len(s) {
			out.WriteByte(s[i])
			continue
		}
		switch s[i+1] {
		case '\\':
			out.WriteByte('\\')
			i++
		case 'x':
			if i+3 < len(s) {
				if n, err := strconv.ParseUint(s[i+2:i+4], 16, 8); err == nil {
					out.WriteByte(byte(n))
					i += 3
					continue
				}
			}
			out.WriteByte(s[i])
		default:
			out.WriteByte(s[i])
		}
	}
	return out.String()
}

// parseFileURL extracts the path from a file://host/path OSC 7 payload.
func parseFileURL(raw string) string {
	u, err := url.Parse(strings.TrimSpace(raw))
	if err != nil || u.Scheme != "file" {
		return ""
	}
	if p, err := url.PathUnescape(u.Path); err == nil {
		return p
	}
	return u.Path
}

// Strip removes ANSI escape sequences from s, returning printable text. It is
// used when handing terminal output to the local model.
func Strip(s string) string {
	sc := NewScanner(func(Event) {}, len(s)+1)
	sc.StartCapture()
	sc.Scan([]byte(s))
	text, _ := sc.StopCapture()
	return text
}
