package vt

import (
	"encoding/base64"
	"testing"
)

func collect(t *testing.T, chunks ...string) []Event {
	t.Helper()
	var events []Event
	s := NewScanner(func(e Event) { events = append(events, e) }, 0)
	for _, c := range chunks {
		s.Scan([]byte(c))
	}
	return events
}

func TestOSC133Cycle(t *testing.T) {
	events := collect(t,
		"\x1b]133;A\x07prompt$ ",
		"\x1b]6973;cmd;"+base64.StdEncoding.EncodeToString([]byte("ls -la"))+"\x07",
		"\x1b]133;C\x07file1\nfile2\n",
		"\x1b]133;D;0\x07",
	)
	want := []EventKind{PromptStart, CommandLine, OutputStart, CommandEnd}
	if len(events) != len(want) {
		t.Fatalf("got %d events, want %d: %+v", len(events), len(want), events)
	}
	for i, k := range want {
		if events[i].Kind != k {
			t.Errorf("event %d: got kind %d, want %d", i, events[i].Kind, k)
		}
	}
	if events[1].Text != "ls -la" {
		t.Errorf("command line = %q", events[1].Text)
	}
	if events[3].ExitCode != 0 {
		t.Errorf("exit = %d", events[3].ExitCode)
	}
}

func TestOSCSplitAcrossChunks(t *testing.T) {
	events := collect(t, "\x1b]13", "3;D;42\x07")
	if len(events) != 1 || events[0].Kind != CommandEnd || events[0].ExitCode != 42 {
		t.Fatalf("split OSC not reassembled: %+v", events)
	}
}

func TestOSCSTTerminator(t *testing.T) {
	events := collect(t, "\x1b]133;A\x1b\\")
	if len(events) != 1 || events[0].Kind != PromptStart {
		t.Fatalf("ESC-backslash terminator not handled: %+v", events)
	}
}

func TestOSC7CWD(t *testing.T) {
	events := collect(t, "\x1b]7;file://myhost/home/user/proj%20x\x07")
	if len(events) != 1 || events[0].Kind != CWDChanged || events[0].Text != "/home/user/proj x" {
		t.Fatalf("OSC 7: %+v", events)
	}
}

func TestCaptureStripsANSI(t *testing.T) {
	s := NewScanner(func(Event) {}, 0)
	s.StartCapture()
	s.Scan([]byte("\x1b[1;32mgreen\x1b[0m text\r\nnext\x1b]0;title\x07line"))
	got, trunc := s.StopCapture()
	if trunc {
		t.Fatal("unexpected truncation")
	}
	if got != "green text\nnextline" {
		t.Fatalf("captured %q", got)
	}
}

func TestCaptureCap(t *testing.T) {
	s := NewScanner(func(Event) {}, 8)
	s.StartCapture()
	s.Scan([]byte("0123456789abcdef"))
	got, trunc := s.StopCapture()
	if !trunc || got != "01234567" {
		t.Fatalf("cap not enforced: %q trunc=%v", got, trunc)
	}
}

func TestUnescape633(t *testing.T) {
	ev, ok := ParseOSC(`633;E;echo \x3bhi\\there`)
	if !ok || ev.Text != `echo ;hi\there` {
		t.Fatalf("633;E decode: %+v ok=%v", ev, ok)
	}
}

func TestStrip(t *testing.T) {
	if got := Strip("\x1b[31mred\x1b[0m"); got != "red" {
		t.Fatalf("Strip = %q", got)
	}
}
