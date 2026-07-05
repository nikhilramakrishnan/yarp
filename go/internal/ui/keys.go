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
)

// DecodeKeys turns a raw stdin chunk into keys, returning any trailing bytes
// that need more input to decode (split escape sequences / UTF-8 runes).
func DecodeKeys(buf []byte) (keys []Key, rest []byte) {
	i := 0
	for i < len(buf) {
		b := buf[i]
		switch {
		case b == 0x1b:
			key, n, need := decodeEscape(buf[i:])
			if need {
				return keys, buf[i:]
			}
			if n == 0 { // lone ESC
				keys = append(keys, Key{Kind: KeyEsc})
				i++
				continue
			}
			keys = append(keys, key)
			i += n
		case b == '\r' || b == '\n':
			keys = append(keys, Key{Kind: KeyEnter})
			i++
		case b == 0x7f || b == 0x08:
			keys = append(keys, Key{Kind: KeyBackspace})
			i++
		case b == '\t':
			keys = append(keys, Key{Kind: KeyTab})
			i++
		case b < 0x20:
			keys = append(keys, Key{Kind: KeyCtrl, Ctrl: b})
			i++
		default:
			r, n := utf8.DecodeRune(buf[i:])
			if r == utf8.RuneError && n == 1 && !utf8.FullRune(buf[i:]) {
				return keys, buf[i:] // wait for the rest of the rune
			}
			keys = append(keys, Key{Kind: KeyRune, Rune: r})
			i += n
		}
	}
	return keys, nil
}

// decodeEscape parses one escape sequence at the start of buf (buf[0]==ESC).
// need=true means the sequence is incomplete. n==0 with need=false means
// treat as a lone ESC key.
func decodeEscape(buf []byte) (key Key, n int, need bool) {
	if len(buf) == 1 {
		// Could be a lone Esc press or a split sequence; treating it as Esc
		// keeps the overlay responsive and terminals rarely split here.
		return Key{}, 0, false
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
		if j > 16 { // runaway; drop it
			return Key{Kind: KeyEsc}, j + 1, false
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
	return Key{Kind: KeyEsc}
}
