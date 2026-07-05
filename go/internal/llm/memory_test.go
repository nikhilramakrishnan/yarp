package llm

import (
	"path/filepath"
	"testing"
	"time"
)

func TestMemoryRememberRecall(t *testing.T) {
	m := OpenMemory(filepath.Join(t.TempDir(), "memory.jsonl"))
	now := time.Now()
	m.Remember("the deploy target is fly.io", now.Add(-time.Hour))
	m.Remember("user prefers zsh with starship prompt", now.Add(-time.Minute))
	m.Remember("project uses postgres 16", now)

	got, err := m.Recall("how do I deploy this?", 2, now)
	if err != nil {
		t.Fatal(err)
	}
	if len(got) == 0 || got[0].Text != "the deploy target is fly.io" {
		t.Fatalf("recall: %+v", got)
	}
}

func TestRecallEmptyStore(t *testing.T) {
	m := OpenMemory(filepath.Join(t.TempDir(), "none.jsonl"))
	got, err := m.Recall("anything", 3, time.Now())
	if err != nil || got != nil {
		t.Fatalf("empty store: %v %v", got, err)
	}
}

func TestAllOrdersOldestFirst(t *testing.T) {
	m := OpenMemory(filepath.Join(t.TempDir(), "m.jsonl"))
	now := time.Now()
	m.Remember("first", now)
	m.Remember("second", now)
	all, err := m.All()
	if err != nil || len(all) != 2 || all[0].Text != "first" {
		t.Fatalf("all: %+v err=%v", all, err)
	}
}
