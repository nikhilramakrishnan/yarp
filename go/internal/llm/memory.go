package llm

import (
	"bufio"
	"encoding/json"
	"os"
	"sort"
	"strings"
	"time"
)

// MemoryEntry is one remembered fact, appended to ~/.yarp/memory.jsonl.
// The file is plain JSONL on purpose: the user can read, edit, or delete
// their agent's memory with a text editor.
type MemoryEntry struct {
	At   time.Time `json:"at"`
	Text string    `json:"text"`
}

// Memory is an append-only fact store with keyword retrieval. It is small
// and dependency-free by design — good enough to make a local model feel
// continuous across sessions without a vector database.
type Memory struct {
	path string
}

// OpenMemory returns the store backed by path.
func OpenMemory(path string) *Memory { return &Memory{path: path} }

// Remember appends a fact.
func (m *Memory) Remember(text string, now time.Time) error {
	f, err := os.OpenFile(m.path, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o600)
	if err != nil {
		return err
	}
	defer f.Close()
	line, err := json.Marshal(MemoryEntry{At: now, Text: strings.TrimSpace(text)})
	if err != nil {
		return err
	}
	_, err = f.Write(append(line, '\n'))
	return err
}

// All returns every entry, oldest first.
func (m *Memory) All() ([]MemoryEntry, error) {
	f, err := os.Open(m.path)
	if os.IsNotExist(err) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	defer f.Close()
	var out []MemoryEntry
	sc := bufio.NewScanner(f)
	sc.Buffer(make([]byte, 0, 64*1024), 1024*1024)
	for sc.Scan() {
		var e MemoryEntry
		if json.Unmarshal(sc.Bytes(), &e) == nil && e.Text != "" {
			out = append(out, e)
		}
	}
	return out, sc.Err()
}

// Recall returns up to k entries relevant to query, scored by keyword
// overlap with a recency tiebreak.
func (m *Memory) Recall(query string, k int, now time.Time) ([]MemoryEntry, error) {
	all, err := m.All()
	if err != nil || len(all) == 0 {
		return nil, err
	}
	qTokens := tokens(query)
	type scored struct {
		e MemoryEntry
		s float64
	}
	var ranked []scored
	for _, e := range all {
		overlap := 0
		eTokens := tokens(e.Text)
		for t := range qTokens {
			if eTokens[t] {
				overlap++
			}
		}
		age := now.Sub(e.At).Hours() + 1
		score := float64(overlap) + 1/age // recency is a weak tiebreak
		if overlap > 0 || len(qTokens) == 0 {
			ranked = append(ranked, scored{e, score})
		}
	}
	sort.SliceStable(ranked, func(i, j int) bool { return ranked[i].s > ranked[j].s })
	if len(ranked) > k {
		ranked = ranked[:k]
	}
	out := make([]MemoryEntry, len(ranked))
	for i, r := range ranked {
		out[i] = r.e
	}
	return out, nil
}

func tokens(s string) map[string]bool {
	out := map[string]bool{}
	for _, w := range strings.FieldsFunc(strings.ToLower(s), func(r rune) bool {
		return !('a' <= r && r <= 'z' || '0' <= r && r <= '9')
	}) {
		if len(w) > 2 {
			out[w] = true
		}
	}
	return out
}
