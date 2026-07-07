package ui

import "testing"

func TestDecodePlainAndControl(t *testing.T) {
	keys, rest := DecodeKeys([]byte("ab\r\x7f\x07"))
	if len(rest) != 0 {
		t.Fatalf("rest = %v", rest)
	}
	want := []Key{
		{Kind: KeyRune, Rune: 'a'},
		{Kind: KeyRune, Rune: 'b'},
		{Kind: KeyEnter},
		{Kind: KeyBackspace},
		{Kind: KeyCtrl, Ctrl: 0x07},
	}
	if len(keys) != len(want) {
		t.Fatalf("got %d keys: %+v", len(keys), keys)
	}
	for i := range want {
		if keys[i] != want[i] {
			t.Errorf("key %d = %+v, want %+v", i, keys[i], want[i])
		}
	}
}

func TestDecodeArrowsAndNav(t *testing.T) {
	keys, _ := DecodeKeys([]byte("\x1b[A\x1b[B\x1b[5~\x1b[3~"))
	want := []KeyKind{KeyUp, KeyDown, KeyPgUp, KeyDelete}
	if len(keys) != len(want) {
		t.Fatalf("got %+v", keys)
	}
	for i, k := range want {
		if keys[i].Kind != k {
			t.Errorf("key %d kind = %d, want %d", i, keys[i].Kind, k)
		}
	}
}

func TestDecodeSplitSequenceWaits(t *testing.T) {
	keys, rest := DecodeKeys([]byte("x\x1b["))
	if len(keys) != 1 || keys[0].Rune != 'x' {
		t.Fatalf("keys = %+v", keys)
	}
	if string(rest) != "\x1b[" {
		t.Fatalf("rest = %q", rest)
	}
	keys, rest = DecodeKeys(append(rest, 'A'))
	if len(keys) != 1 || keys[0].Kind != KeyUp || len(rest) != 0 {
		t.Fatalf("continuation failed: %+v %q", keys, rest)
	}
}

// A lone trailing ESC is ambiguous (Esc keypress vs split sequence); the
// decoder must ask for more input and let the session's timer disambiguate,
// otherwise a split arrow key closes the overlay and leaks "[A" to the shell.
func TestLoneTrailingEscWaits(t *testing.T) {
	keys, rest := DecodeKeys([]byte("\x1b"))
	if len(keys) != 0 || string(rest) != "\x1b" {
		t.Fatalf("lone ESC should wait: keys=%+v rest=%q", keys, rest)
	}
}

// Paste markers and focus events must be inert — decoding them as Esc used
// to dismiss the overlay whenever the user pasted.
func TestUnknownCSIIsIgnored(t *testing.T) {
	for _, seq := range []string{"\x1b[200~", "\x1b[201~", "\x1b[I", "\x1b[O"} {
		keys, rest := DecodeKeys([]byte(seq))
		if len(rest) != 0 || len(keys) != 1 || keys[0].Kind != KeyIgnore {
			t.Fatalf("%q decoded to %+v (rest %q), want KeyIgnore", seq, keys, rest)
		}
	}
}

func TestDecodeUTF8(t *testing.T) {
	keys, rest := DecodeKeys([]byte("héπ"))
	if len(rest) != 0 || len(keys) != 3 || keys[2].Rune != 'π' {
		t.Fatalf("utf8: %+v rest=%q", keys, rest)
	}
}

func TestWrapText(t *testing.T) {
	lines := wrapText("aaaa bbbb cccc", 9)
	if len(lines) != 2 || lines[0] != "aaaa " && lines[0] != "aaaa bbbb" {
		t.Fatalf("wrap: %q", lines)
	}
}

func TestClipVisiblePreservesSGR(t *testing.T) {
	got := clipVisible("\x1b[1mhello world", 5)
	if got != "\x1b[1mhello" {
		t.Fatalf("clip = %q", got)
	}
}
