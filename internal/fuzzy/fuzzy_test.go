package fuzzy

import "testing"

func TestMatchBasics(t *testing.T) {
	if _, ok := Match("gst", "git status"); !ok {
		t.Fatal("subsequence should match")
	}
	if _, ok := Match("zzz", "git status"); ok {
		t.Fatal("non-subsequence should not match")
	}
	if _, ok := Match("", "anything"); !ok {
		t.Fatal("empty query matches everything")
	}
}

func TestWordStartsBeatScattered(t *testing.T) {
	wordStart, _ := Match("gs", "git status")
	scattered, _ := Match("gs", "grep -rns")
	if wordStart <= scattered {
		t.Fatalf("word-start score %d should beat scattered %d", wordStart, scattered)
	}
}

func TestRankOrdersAndFilters(t *testing.T) {
	items := []string{"cargo build", "git stash", "git status"}
	ranked := Rank("gst", items, func(s string) string { return s })
	if len(ranked) != 2 {
		t.Fatalf("expected 2 matches, got %d", len(ranked))
	}
	if ranked[0].Item != "git status" && ranked[0].Item != "git stash" {
		t.Fatalf("unexpected top item %q", ranked[0].Item)
	}
}
