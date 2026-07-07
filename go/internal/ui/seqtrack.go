package ui

// seqTracker follows escape-sequence structure in the stdin stream during
// passthrough. The palette hotkey may only fire on a byte typed in ground
// state: terminal→application replies (OSC color reports, DA responses)
// legitimately contain BEL and ESC, and bracketed pastes can contain
// anything. Without this, a child app querying the terminal would pop the
// overlay open and have its reply garbled.
type seqTracker struct {
	state seqState
	paste bool
	param []byte // CSI parameter bytes, kept to spot paste markers
}

type seqState int

const (
	seqGround seqState = iota
	seqEsc
	seqCSI
	seqOSC
	seqOSCEsc
	seqStr
	seqStrEsc
)

// ground reports whether the next byte would be ordinary input.
func (t *seqTracker) ground() bool { return t.state == seqGround && !t.paste }

// feed advances the tracker by one byte.
func (t *seqTracker) feed(b byte) {
	switch t.state {
	case seqGround:
		if b == 0x1b {
			t.state = seqEsc
		}
	case seqEsc:
		switch b {
		case '[':
			t.state = seqCSI
			t.param = t.param[:0]
		case ']':
			t.state = seqOSC
		case 'P', 'X', '^', '_':
			t.state = seqStr
		default:
			t.state = seqGround
		}
	case seqCSI:
		if b >= 0x40 && b <= 0x7e {
			// Bracketed paste brackets everything until its close marker.
			if b == '~' {
				switch string(t.param) {
				case "200":
					t.paste = true
				case "201":
					t.paste = false
				}
			}
			t.state = seqGround
		} else if len(t.param) < 8 {
			t.param = append(t.param, b)
		}
	case seqOSC:
		switch b {
		case 0x07:
			t.state = seqGround
		case 0x1b:
			t.state = seqOSCEsc
		}
	case seqOSCEsc:
		if b == '\\' {
			t.state = seqGround
		} else if b != 0x1b {
			t.state = seqOSC
		}
	case seqStr:
		if b == 0x1b {
			t.state = seqStrEsc
		}
	case seqStrEsc:
		if b == '\\' {
			t.state = seqGround
		} else if b != 0x1b {
			t.state = seqStr
		}
	}
}

// reset returns to ground; used when the overlay interrupts the stream.
func (t *seqTracker) reset() {
	t.state = seqGround
	t.paste = false
}
