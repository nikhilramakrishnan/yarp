package semantic

import (
	"encoding/base64"
	"testing"
	"time"
)

type fakeClock struct {
	now time.Time
}

func (c *fakeClock) tick() time.Time {
	now := c.now
	c.now = c.now.Add(time.Second)
	return now
}

func TestObserverEmitsStableRunningAndCompletedUpdates(t *testing.T) {
	clock := &fakeClock{now: time.Date(2026, 7, 19, 12, 0, 0, 0, time.UTC)}
	var updates []Update
	o := NewObserver("session-a", 1024, clock.tick, func(update Update) {
		updates = append(updates, update)
	})

	command := base64.StdEncoding.EncodeToString([]byte("printf 'hi\\n'"))
	o.Scan([]byte("\x1b]7;file://localhost/tmp/project\x07"))
	// Deliberately split metadata across chunks. The
	// underlying scanner is streaming and must preserve one lifecycle.
	o.Scan([]byte("\x1b]6973;cmd;" + command + "\x07\x1b]133;"))
	o.Scan([]byte("C\x07\x1b[32mhi\x1b[0m\r\n"))
	o.Scan([]byte("\x1b]133;D;0\x07"))

	if len(updates) != 2 {
		t.Fatalf("got %d updates, want running + completed: %+v", len(updates), updates)
	}
	running, completed := updates[0], updates[1]
	if running.ID != completed.ID || running.ID != "session-a:000001" {
		t.Fatalf("unstable block ID: %q then %q", running.ID, completed.ID)
	}
	if running.Revision != 1 || running.Status != StatusRunning {
		t.Fatalf("running update = %+v", running)
	}
	if completed.Revision != 2 || completed.Status != StatusCompleted || !completed.Terminal() {
		t.Fatalf("completed update = %+v", completed)
	}
	if completed.Record.Cmd != "printf 'hi\\n'" || completed.Record.CWD != "/tmp/project" {
		t.Fatalf("metadata = %+v", completed.Record)
	}
	if completed.Record.Output != "hi" || completed.Record.ExitCode != 0 || completed.Record.Truncated {
		t.Fatalf("result = %+v", completed.Record)
	}
	if completed.Record.EndedAt.Sub(completed.Record.StartedAt) != time.Second {
		t.Fatalf("duration = %s", completed.Record.Duration())
	}
}

func TestObserverMarksNonzeroAndTruncatedOutput(t *testing.T) {
	now := time.Date(2026, 7, 19, 12, 0, 0, 0, time.UTC)
	var updates []Update
	o := NewObserver("session-b", 4, func() time.Time { return now }, func(update Update) {
		updates = append(updates, update)
	})
	o.Scan([]byte("\x1b]6973;cmd;" + base64.StdEncoding.EncodeToString([]byte("false")) + "\x07"))
	o.Scan([]byte("\x1b]133;C\x07abcdefgh\x1b]133;D;7\x07"))

	got := updates[len(updates)-1]
	if got.Status != StatusFailed || got.Record.ExitCode != 7 {
		t.Fatalf("failed block = %+v", got)
	}
	if got.Record.Output != "abcd" || !got.Record.Truncated {
		t.Fatalf("truncation = %+v", got.Record)
	}
}

func TestObserverSupportsPowerShellDelayedMetadata(t *testing.T) {
	now := time.Date(2026, 7, 19, 12, 0, 0, 0, time.UTC)
	var updates []Update
	o := NewObserver("pwsh", 1024, func() time.Time { return now }, func(update Update) {
		updates = append(updates, update)
	})
	o.Scan([]byte("\x1b]7;file://host/C:/work\x07"))
	o.Scan([]byte("\x1b]6973;cmd;" + base64.StdEncoding.EncodeToString([]byte("Get-Item missing")) + "\x07"))
	o.Scan([]byte("\x1b]133;D;1\x07\x1b]133;A\x07"))

	if len(updates) != 1 {
		t.Fatalf("got updates %+v", updates)
	}
	got := updates[0]
	if got.Status != StatusFailed || got.Record.Cmd != "Get-Item missing" || got.Record.Output != "" {
		t.Fatalf("PowerShell block = %+v", got)
	}
	if !got.Record.StartedAt.Equal(got.Record.EndedAt) {
		t.Fatalf("delayed block should have unknown/zero duration: %+v", got.Record)
	}
}

func TestObserverCancelsInterruptedAndEndedBlocks(t *testing.T) {
	now := time.Date(2026, 7, 19, 12, 0, 0, 0, time.UTC)
	var updates []Update
	o := NewObserver("session-c", 1024, func() time.Time { return now }, func(update Update) {
		updates = append(updates, update)
	})
	o.Scan([]byte("\x1b]133;C\x07partial"))
	o.Scan([]byte("\x1b]133;A\x07"))
	o.Scan([]byte("\x1b]133;C\x07more"))
	o.Flush()
	o.Flush()

	if len(updates) != 4 {
		t.Fatalf("got updates %+v", updates)
	}
	if updates[1].Status != StatusCancelled || updates[1].Reason != "prompt-restarted" || updates[1].Record.Output != "partial" {
		t.Fatalf("prompt cancellation = %+v", updates[1])
	}
	if updates[3].Status != StatusCancelled || updates[3].Reason != "session-ended" || updates[3].Record.Output != "more" {
		t.Fatalf("flush cancellation = %+v", updates[3])
	}
}

func TestSupersedingBoundaryKeepsNewCommandMetadata(t *testing.T) {
	now := time.Date(2026, 7, 19, 12, 0, 0, 0, time.UTC)
	var updates []Update
	o := NewObserver("session-e", 1024, func() time.Time { return now }, func(update Update) {
		updates = append(updates, update)
	})
	o.Scan([]byte("\x1b]6973;cmd;" + base64.StdEncoding.EncodeToString([]byte("old")) + "\x07"))
	o.Scan([]byte("\x1b]133;C\x07partial"))
	o.Scan([]byte("\x1b]6973;cmd;" + base64.StdEncoding.EncodeToString([]byte("new")) + "\x07"))
	o.Scan([]byte("\x1b]133;C\x07done\x1b]133;D;0\x07"))

	if len(updates) != 4 {
		t.Fatalf("got updates %+v", updates)
	}
	if updates[1].Status != StatusCancelled || updates[1].Record.Cmd != "old" {
		t.Fatalf("interrupted block = %+v", updates[1])
	}
	if updates[2].Status != StatusRunning || updates[2].Record.Cmd != "new" {
		t.Fatalf("replacement block = %+v", updates[2])
	}
	if updates[3].Status != StatusCompleted || updates[3].Record.Cmd != "new" || updates[3].Record.Output != "done" {
		t.Fatalf("replacement completion = %+v", updates[3])
	}
}

func TestCommandLineAfterOutputStartRevisesRunningBlock(t *testing.T) {
	now := time.Date(2026, 7, 19, 12, 0, 0, 0, time.UTC)
	var updates []Update
	o := NewObserver("session-d", 1024, func() time.Time { return now }, func(update Update) {
		updates = append(updates, update)
	})
	o.Scan([]byte("\x1b]133;C\x07"))
	o.Scan([]byte("\x1b]633;E;echo \\x3b ok\x07"))
	o.Scan([]byte("done\x1b]133;D;0\x07"))

	if len(updates) != 3 {
		t.Fatalf("got updates %+v", updates)
	}
	if updates[0].Record.Cmd != "" || updates[1].Record.Cmd != "echo ; ok" {
		t.Fatalf("late command update = %+v", updates)
	}
	if updates[2].ID != updates[0].ID || updates[2].Revision != 3 {
		t.Fatalf("revision/identity = %+v", updates)
	}
}
