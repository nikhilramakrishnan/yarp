// Package fuzzy implements the subsequence matcher behind the palette.
// It replaces the Rust fuzzy_match crate with a small scorer tuned for
// command lines: word starts and consecutive runs win, scattered matches lose.
package fuzzy

import (
	"sort"
	"strings"
	"unicode"
)

// Match scores candidate against query. ok is false when query is not a
// subsequence of candidate. Higher scores are better.
func Match(query, candidate string) (score int, ok bool) {
	if query == "" {
		return 0, true
	}
	q := strings.ToLower(query)
	c := strings.ToLower(candidate)
	qi := 0
	prevHit := -2
	for ci := 0; ci < len(c) && qi < len(q); ci++ {
		if c[ci] != q[qi] {
			continue
		}
		score += 1
		if ci == 0 || isWordBoundary(rune(c[ci-1])) {
			score += 8 // word-start hit
		}
		if ci == prevHit+1 {
			score += 4 // consecutive run
		}
		prevHit = ci
		qi++
	}
	if qi < len(q) {
		return 0, false
	}
	// Prefer shorter candidates when hits are equal.
	score -= len(c) / 8
	return score, true
}

func isWordBoundary(r rune) bool {
	return unicode.IsSpace(r) || strings.ContainsRune("-_/.:", r)
}

// Ranked pairs an item with its score.
type Ranked[T any] struct {
	Item  T
	Score int
}

// Rank filters and sorts items by fuzzy score against query, using key to
// extract the searchable text. Stable for equal scores.
func Rank[T any](query string, items []T, key func(T) string) []Ranked[T] {
	out := make([]Ranked[T], 0, len(items))
	for _, it := range items {
		if s, ok := Match(query, key(it)); ok {
			out = append(out, Ranked[T]{Item: it, Score: s})
		}
	}
	sort.SliceStable(out, func(i, j int) bool { return out[i].Score > out[j].Score })
	return out
}
