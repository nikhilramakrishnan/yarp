package ui

import (
	"fmt"
	"strings"
)

// Minimal ANSI rendering helpers. The overlay is drawn on the host
// terminal's alternate screen with plain escape codes — no TUI framework,
// no event loop of its own, nothing between yarp and the terminal.

const (
	altScreenOn  = "\x1b[?1049h\x1b[?25l"
	altScreenOff = "\x1b[?1049l\x1b[?25h"
	clearScreen  = "\x1b[2J\x1b[H"

	sgrReset   = "\x1b[0m"
	sgrBold    = "\x1b[1m"
	sgrDim     = "\x1b[2m"
	sgrInverse = "\x1b[7m"
	sgrRed     = "\x1b[31m"
	sgrGreen   = "\x1b[32m"
	sgrYellow  = "\x1b[33m"
	sgrCyan    = "\x1b[36m"
)

// frame accumulates one overlay repaint.
type frame struct {
	b    strings.Builder
	cols int
	rows int
}

func newFrame(cols, rows int) *frame {
	f := &frame{cols: cols, rows: rows}
	f.b.WriteString(clearScreen)
	return f
}

// line writes one row, clipped to the terminal width (rune-aware, assumes
// narrow glyphs — wide CJK may wrap a little early, never corrupt).
func (f *frame) line(row int, s string) {
	fmt.Fprintf(&f.b, "\x1b[%d;1H", row+1)
	f.b.WriteString(clipVisible(s, f.cols))
	f.b.WriteString(sgrReset)
}

func (f *frame) String() string { return f.b.String() }

// clipVisible truncates s to width printable columns while passing SGR
// escape sequences through unmeasured.
func clipVisible(s string, width int) string {
	var out strings.Builder
	visible := 0
	inEsc := false
	for _, r := range s {
		switch {
		case inEsc:
			out.WriteRune(r)
			if (r >= 0x40 && r <= 0x7e) && r != '[' {
				inEsc = false
			}
		case r == 0x1b:
			inEsc = true
			out.WriteRune(r)
		default:
			if visible >= width {
				continue
			}
			out.WriteRune(r)
			visible++
		}
	}
	return out.String()
}

// wrapText hard-wraps plain text to width, preserving existing newlines.
func wrapText(s string, width int) []string {
	if width < 4 {
		width = 4
	}
	var out []string
	for _, raw := range strings.Split(s, "\n") {
		if raw == "" {
			out = append(out, "")
			continue
		}
		for len(raw) > 0 {
			runes := []rune(raw)
			if len(runes) <= width {
				out = append(out, raw)
				break
			}
			cut := width
			// Prefer a space break near the end of the line.
			for k := width; k > width/2; k-- {
				if runes[k-1] == ' ' {
					cut = k
					break
				}
			}
			out = append(out, string(runes[:cut]))
			raw = strings.TrimLeft(string(runes[cut:]), " ")
		}
	}
	return out
}
