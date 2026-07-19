package blocks

import (
	"testing"
	"time"
)

func TestStoreRoundTrip(t *testing.T) {
	dir := t.TempDir()
	now := time.Date(2026, 7, 5, 12, 0, 0, 0, time.UTC)
	st, err := OpenStore(dir, SessionID(now, 123))
	if err != nil {
		t.Fatal(err)
	}
	for i, cmd := range []string{"ls", "git status", "ls"} {
		if err := st.Append(&Block{
			Cmd: cmd, CWD: "/tmp", ExitCode: i,
			StartedAt: now, EndedAt: now.Add(time.Second),
		}); err != nil {
			t.Fatal(err)
		}
	}
	if err := st.Close(); err != nil {
		t.Fatal(err)
	}

	recent, err := LoadRecent(dir, 10)
	if err != nil {
		t.Fatal(err)
	}
	if len(recent) != 3 {
		t.Fatalf("got %d blocks", len(recent))
	}
	if recent[0].Cmd != "ls" || recent[0].ExitCode != 2 {
		t.Fatalf("newest first expected, got %+v", recent[0])
	}
	if recent[0].Duration() != time.Second {
		t.Fatalf("duration = %v", recent[0].Duration())
	}

	cmds, err := Commands(dir, 10)
	if err != nil {
		t.Fatal(err)
	}
	if len(cmds) != 2 || cmds[0] != "ls" || cmds[1] != "git status" {
		t.Fatalf("dedup failed: %v", cmds)
	}
}

func TestPrune(t *testing.T) {
	dir := t.TempDir()
	now := time.Now()
	st, err := OpenStore(dir, "20200101-000000-1")
	if err != nil {
		t.Fatal(err)
	}
	st.Append(&Block{Cmd: "old"})
	st.Close()

	removed, err := Prune(dir, time.Hour, now.Add(48*time.Hour))
	if err != nil || removed != 1 {
		t.Fatalf("removed=%d err=%v", removed, err)
	}
	recent, _ := LoadRecent(dir, 10)
	if len(recent) != 0 {
		t.Fatalf("expected empty history, got %d", len(recent))
	}
}

func TestLoadRecentMissingDir(t *testing.T) {
	recent, err := LoadRecent("/nonexistent/path/for/test", 10)
	if err != nil || recent != nil {
		t.Fatalf("missing dir should be empty: %v %v", recent, err)
	}
}
