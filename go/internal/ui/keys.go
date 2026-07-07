package ui

import "unicode/utf8"

// Key is a decoded keypress for overlay input. Passthrough mode never
// decodes keys — bytes go straight to the pty — so this only needs the small
// vocabulary the overlay uses.
type Key struct {
	Kind KeyKind
	Rune rune // KeyRune only
	Ctrl byte // KeyCtrl only: 1..26
}

// KeyKind classifies a keypress.
type KeyKind int

const (
	KeyRune KeyKind = iota
	KeyCtrl
	KeyEnter
	KeyEsc
	KeyBackspace
	KeyTab
	KeyUp
	KeyDown
	KeyLeft
	KeyRight
	KeyPgUp
	KeyPgDn
	KeyHome
	KeyEnd
	KeyDelete
	// KeyIgnore is a decoded-but-meaningless sequence: bracketed-paste
	// markers, focus events, mouse reports, unknown CSI. Overlay handlers
	// must not react to it — in particular it must never alias to Esc, or
	// pasting into the palette would dismiss the overlay.
	KeyIgnore
)

// DecodeOne decodes the first key in buf, returning the key, the number of
// bytes consumed, and need=true when buf ends mid-sequence (partial escape
// sequence or UTF-8 rune) and more input is required. A lone trailing ESC
// reports need=true too: the caller disambiguates "Esc keypress" from "start
// of a split sequence" with a short timer, because the bytes alone can't.
func DecodeOne(buf []byte) (key Key, n int, need bool) {
	if len(buf) == 0 {
		return Key{}, 0, true
	}
	b := buf[0]
	switch {
	case b == 0x1b:
		return decodeEscape(buf)
	case b == '\r' || b == '\n':
		return Key{Kind: KeyEnter}, 1, false
	case b == 0x7f || b == 0x08:
		return Key{Kind: KeyBackspace}, 1, false
	case b == '\t':
		return Key{Kind: KeyTab}, 1, false
	case b < 0x20:
		return Key{Kind: KeyCtrl, Ctrl: b}, 1, false
	default:
		r, size := utf8.DecodeRune(buf)
		if r == utf8.RuneError && size == 1 && !utf8.FullRune(buf) {
			return Key{}, 0, true // wait for the rest of the rune
		}
		return Key{Kind: KeyRune, Rune: r}, size, false
	}
}

// DecodeKeys decodes as many keys as buf holds, returning trailing bytes
// that need more input.
func DecodeKeys(buf []byte) (keys []Key, rest []byte) {
	i := 0
	for i < len(buf) {
		k, n, need := DecodeOne(buf[i:])
		if need {
			return keys, buf[i:]
		}
		keys = append(keys, k)
		i += n
	}
	return keys, nil
}

// decodeEscape parses one escape sequence at the start of buf (buf[0]==ESC).
func decodeEscape(buf []byte) (key Key, n int, need bool) {
	if len(buf) == 1 {
		return Key{}, 0, true // lone ESC or split sequence; caller decides
	}
	if buf[1] != '[' && buf[1] != 'O' {
		// Alt+<key> or other ESC-prefixed input; report Esc and let the next
		// pass handle the remaining byte.
		return Key{Kind: KeyEsc}, 1, false
	}
	// CSI / SS3: ESC [ <params> <final> where final is 0x40..0x7e.
	for j := 2; j < len(buf); j++ {
		c := buf[j]
		if c >= 0x40 && c <= 0x7e {
			return csiKey(buf[2:j], c), j + 1, false
		}
		if j > 24 { // runaway; drop it
			return Key{Kind: KeyIgnore}, j + 1, false
		}
	}
	return Key{}, 0, true
}

func csiKey(params []byte, final byte) Key {
	switch final {
	case 'A':
		return Key{Kind: KeyUp}
	case 'B':
		return Key{Kind: KeyDown}
	case 'C':
		return Key{Kind: KeyRight}
	case 'D':
		return Key{Kind: KeyLeft}
	case 'H':
		return Key{Kind: KeyHome}
	case 'F':
		return Key{Kind: KeyEnd}
	case '~':
		switch string(params) {
		case "1", "7":
			return Key{Kind: KeyHome}
		case "3":
			return Key{Kind: KeyDelete}
		case "4", "8":
			return Key{Kind: KeyEnd}
		case "5":
			return Key{Kind: KeyPgUp}
		case "6":
			return Key{Kind: KeyPgDn}
		}
	}
	// Everything else — paste markers 200~/201~, focus I/O, mouse, unknown
	// CSI — is deliberately inert.
	return Key{Kind: KeyIgnore}
}
