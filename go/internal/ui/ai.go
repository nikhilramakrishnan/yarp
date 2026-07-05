package ui

import (
	"context"
	"encoding/base64"
	"strings"
	"time"

	"github.com/nikhilramakrishnan/yarp/go/internal/agent"
	"github.com/nikhilramakrishnan/yarp/go/internal/backup"
	"github.com/nikhilramakrishnan/yarp/go/internal/config"
	"github.com/nikhilramakrishnan/yarp/go/internal/llm"
)

// AI pane wiring. The model call runs in its own goroutine; deltas and the
// final result come back through session channels so all state mutation
// stays on the main loop.

type aiResult struct {
	text string
	err  error
}

func (s *Session) keyAI(k Key) {
	o := s.ov
	switch k.Kind {
	case KeyEsc:
		if s.aiCancel != nil {
			s.aiCancel()
			s.aiCancel = nil
			o.streaming = false
			o.chat = append(o.chat, chatLine{role: "note", text: "cancelled"})
			return
		}
		o.mode = modePalette
	case KeyBackspace:
		if o.aiInput != "" {
			r := []rune(o.aiInput)
			o.aiInput = string(r[:len(r)-1])
		}
	case KeyRune:
		// 1-9 insert a suggested command when the input line is empty.
		if o.aiInput == "" && k.Rune >= '1' && k.Rune <= '9' {
			idx := int(k.Rune - '1')
			if idx < len(o.suggested) {
				s.insertIntoShell(o.suggested[idx])
				return
			}
		}
		o.aiInput += string(k.Rune)
	case KeyEnter:
		q := strings.TrimSpace(o.aiInput)
		if q == "" || o.streaming {
			return
		}
		o.aiInput = ""
		if fact, ok := strings.CutPrefix(q, "/remember "); ok {
			s.rememberFact(fact)
			return
		}
		o.chat = append(o.chat, chatLine{role: "you", text: q})
		s.startAIRequest(q)
	}
}

func (s *Session) rememberFact(fact string) {
	o := s.ov
	if !s.settings.MemoryOn() {
		o.chat = append(o.chat, chatLine{role: "note", text: "memory is disabled in settings"})
		return
	}
	mem := s.memory()
	if err := mem.Remember(fact, time.Now()); err != nil {
		o.chat = append(o.chat, chatLine{role: "note", text: "could not save: " + err.Error()})
		return
	}
	o.chat = append(o.chat, chatLine{role: "note", text: "remembered: " + fact})
}

func (s *Session) memory() *llm.Memory {
	base, _ := config.Dir()
	return llm.OpenMemory(base + "/memory.jsonl")
}

// startAIRequest kicks off one model call. The agent is created lazily on
// the first question and lives for the whole terminal session, so the
// conversation survives overlay open/close.
func (s *Session) startAIRequest(question string) {
	o := s.ov
	o.streaming = true
	o.suggested = nil
	o.chat = append(o.chat, chatLine{role: "yarp", text: ""})

	ctx, cancel := context.WithCancel(context.Background())
	s.aiCancel = cancel

	if s.aiAgent == nil {
		client, err := llm.Discover(ctx, s.settings.LLM.Endpoint, s.settings.LLM.Model)
		if err != nil {
			cancel()
			s.aiCancel = nil
			o.streaming = false
			o.chat = append(o.chat, chatLine{role: "note", text: err.Error()})
			return
		}
		s.aiAgent = &agent.Agent{Client: client}
		if s.settings.MemoryOn() {
			s.aiAgent.Memory = s.memory()
		}
	}

	// Snapshot everything the goroutine needs; it must not touch s.
	recent := s.recentBlocks(s.settings.LLM.ContextBlocks)
	cwd := s.cwd
	ag := s.aiAgent
	delta, done := s.aiDelta, s.aiDone

	go func() {
		defer cancel()
		full, err := ag.Reply(ctx, question, recent, cwd, func(d string) {
			select {
			case delta <- d:
			case <-ctx.Done():
			}
		})
		done <- aiResult{text: full, err: err}
	}()
}

func (s *Session) aiAppendDelta(d string) {
	o := s.ov
	if o == nil || len(o.chat) == 0 {
		return
	}
	last := &o.chat[len(o.chat)-1]
	if last.role == "yarp" {
		last.text += d
	}
}

func (s *Session) aiFinish(res aiResult) {
	s.aiCancel = nil
	o := s.ov
	if o == nil {
		return
	}
	o.streaming = false
	if res.err != nil {
		o.chat = append(o.chat, chatLine{role: "note", text: res.err.Error()})
		return
	}
	// The streamed deltas already populated the transcript; extract any
	// proposed commands and strip REMEMBER lines from display.
	if len(o.chat) > 0 && o.chat[len(o.chat)-1].role == "yarp" {
		o.chat[len(o.chat)-1].text = stripRememberLines(res.text)
	}
	o.suggested = agent.SuggestedCommands(res.text)
}

func stripRememberLines(s string) string {
	var out []string
	for _, l := range strings.Split(s, "\n") {
		if strings.HasPrefix(strings.TrimSpace(l), "REMEMBER:") {
			continue
		}
		out = append(out, l)
	}
	return strings.TrimSpace(strings.Join(out, "\n"))
}

func base64Std(s string) string {
	return base64.StdEncoding.EncodeToString([]byte(s))
}

// runBackup bridges the overlay's "Back up now" action to the backup package.
func runBackup(settings *config.Settings, homeDir string, now time.Time) (string, error) {
	return backup.Run(homeDir, backup.RunConfig{
		Dir:          settings.Backup.Dir,
		RcloneRemote: settings.Backup.RcloneRemote,
		Keep:         settings.Backup.Keep,
		Drive: backup.DriveAuth{
			ClientID:     settings.Backup.GoogleDrive.ClientID,
			ClientSecret: settings.Backup.GoogleDrive.ClientSecret,
			RefreshToken: settings.Backup.GoogleDrive.RefreshToken,
		},
		DriveFolder: settings.Backup.GoogleDrive.FolderID,
	}, now)
}
