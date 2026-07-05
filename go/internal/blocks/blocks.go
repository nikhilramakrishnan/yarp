// Package blocks models Warp-style command blocks and their local persistence.
//
// A block is one prompt→command→output→exit cycle, delimited by the shell
// integration events in package vt. Completed blocks are appended as JSON
// lines to per-session files under ~/.yarp/history/, which keeps the format
// greppable, diffable, and trivially backed up.
package blocks

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

// Block is one completed command cycle.
type Block struct {
	Session   string    `json:"session"`
	Seq       int       `json:"seq"`
	Cmd       string    `json:"cmd"`
	CWD       string    `json:"cwd,omitempty"`
	StartedAt time.Time `json:"started_at"`
	EndedAt   time.Time `json:"ended_at"`
	ExitCode  int       `json:"exit_code"` // -1 when unknown
	Output    string    `json:"output,omitempty"`
	Truncated bool      `json:"truncated,omitempty"`
	Bookmark  bool      `json:"bookmark,omitempty"`
}

// Duration is the block's wall-clock run time.
func (b *Block) Duration() time.Duration {
	if b.EndedAt.IsZero() || b.StartedAt.IsZero() {
		return 0
	}
	return b.EndedAt.Sub(b.StartedAt)
}

// Store appends completed blocks to one JSONL file per session.
type Store struct {
	dir     string
	session string
	file    *os.File
	seq     int
}

// OpenStore creates the session file under historyDir.
func OpenStore(historyDir, session string) (*Store, error) {
	if err := os.MkdirAll(historyDir, 0o700); err != nil {
		return nil, err
	}
	path := filepath.Join(historyDir, session+".jsonl")
	f, err := os.OpenFile(path, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o600)
	if err != nil {
		return nil, err
	}
	return &Store{dir: historyDir, session: session, file: f}, nil
}

// Append persists one block, assigning its sequence number.
func (s *Store) Append(b *Block) error {
	s.seq++
	b.Session = s.session
	b.Seq = s.seq
	line, err := json.Marshal(b)
	if err != nil {
		return err
	}
	_, err = s.file.Write(append(line, '\n'))
	return err
}

// Close flushes and closes the session file.
func (s *Store) Close() error { return s.file.Close() }

// Session returns the store's session identifier.
func (s *Store) Session() string { return s.session }

// SessionID builds a sortable, unique session identifier.
func SessionID(now time.Time, pid int) string {
	return fmt.Sprintf("%s-%d", now.Format("20060102-150405"), pid)
}

// LoadRecent returns up to limit blocks from the newest session files,
// newest block first.
func LoadRecent(historyDir string, limit int) ([]Block, error) {
	files, err := sessionFiles(historyDir)
	if err != nil || len(files) == 0 {
		return nil, err
	}
	var out []Block
	// files are sorted oldest→newest; walk backwards.
	for i := len(files) - 1; i >= 0 && len(out) < limit; i-- {
		blocksInFile, err := readSession(files[i])
		if err != nil {
			continue // a corrupt file must not take history down
		}
		for j := len(blocksInFile) - 1; j >= 0 && len(out) < limit; j-- {
			out = append(out, blocksInFile[j])
		}
	}
	return out, nil
}

// Commands returns distinct commands from recent history, newest first,
// capped at limit. Used to feed the palette.
func Commands(historyDir string, limit int) ([]string, error) {
	blks, err := LoadRecent(historyDir, limit*4)
	if err != nil {
		return nil, err
	}
	seen := make(map[string]bool, len(blks))
	var out []string
	for _, b := range blks {
		cmd := strings.TrimSpace(b.Cmd)
		if cmd == "" || seen[cmd] {
			continue
		}
		seen[cmd] = true
		out = append(out, cmd)
		if len(out) == limit {
			break
		}
	}
	return out, nil
}

// Prune deletes session files whose modification time is older than maxAge.
func Prune(historyDir string, maxAge time.Duration, now time.Time) (int, error) {
	files, err := sessionFiles(historyDir)
	if err != nil {
		return 0, err
	}
	removed := 0
	for _, f := range files {
		info, err := os.Stat(f)
		if err != nil {
			continue
		}
		if now.Sub(info.ModTime()) > maxAge {
			if os.Remove(f) == nil {
				removed++
			}
		}
	}
	return removed, nil
}

func sessionFiles(historyDir string) ([]string, error) {
	entries, err := os.ReadDir(historyDir)
	if os.IsNotExist(err) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	var files []string
	for _, e := range entries {
		if !e.IsDir() && strings.HasSuffix(e.Name(), ".jsonl") {
			files = append(files, filepath.Join(historyDir, e.Name()))
		}
	}
	sort.Strings(files) // names start with a sortable timestamp
	return files, nil
}

func readSession(path string) ([]Block, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	var out []Block
	sc := bufio.NewScanner(f)
	sc.Buffer(make([]byte, 0, 64*1024), 4*1024*1024)
	for sc.Scan() {
		var b Block
		if json.Unmarshal(sc.Bytes(), &b) == nil {
			out = append(out, b)
		}
	}
	return out, sc.Err()
}
