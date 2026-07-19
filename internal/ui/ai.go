package ui

import (
	"context"
	"strings"
	"time"

	"github.com/nikhilramakrishnan/yarp/internal/agent"
	"github.com/nikhilramakrishnan/yarp/internal/config"
	"github.com/nikhilramakrishnan/yarp/internal/llm"
)

// AI pane wiring. The model call runs in its own goroutine; deltas and the
// final result come back through session channels so all state mutation
// stays on the main loop. Every message carries the generation counter of
// the request that produced it: cancelling a request bumps the generation,
// so a cancelled goroutine's stragglers can never leak into the transcript
// of a newer question.

type aiDelta struct {
	gen  int
	text string
}

type aiResult struct {
	gen  int
	text string
	err  error
}

func (s *Session) keyAI(k Key) {
	o := s.ov
	switch k.Kind {
	case KeyEsc:
		if s.aiCancel != nil {
			s.cancelAI()
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

// cancelAI aborts the in-flight request and invalidates its generation so
// late deltas/results are dropped on the floor.
func (s *Session) cancelAI() {
	if s.aiCancel != nil {
		s.aiCancel()
		s.aiCancel = nil
	}
	s.aiGen++
	if s.ov != nil {
		s.ov.streaming = false
	}
}

func (s *Session) rememberFact(fact string) {
	o := s.ov
	if !s.settings.MemoryOn() {
		o.chat = append(o.chat, chatLine{role: "note", text: "memory is disabled in settings"})
		return
	}
	mem, err := s.memory()
	if err == nil {
		err = mem.Remember(fact, time.Now())
	}
	if err != nil {
		o.chat = append(o.chat, chatLine{role: "note", text: "could not save: " + err.Error()})
		return
	}
	o.chat = append(o.chat, chatLine{role: "note", text: "remembered: " + fact})
}

func (s *Session) memory() (*llm.Memory, error) {
	path, err := config.MemoryPath()
	if err != nil {
		return nil, err
	}
	return llm.OpenMemory(path), nil
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
	s.aiGen++
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
			if mem, err := s.memory(); err == nil {
				s.aiAgent.Memory = mem
			}
		}
	}

	// Snapshot everything the goroutine needs; it must not touch s.
	gen := s.aiGen
	recent := s.recentBlocks(s.settings.LLM.ContextBlocks)
	cwd := s.cwd
	ag := s.aiAgent
	delta, done := s.aiDelta, s.aiDone

	go func() {
		defer cancel()
		full, err := ag.Reply(ctx, question, recent, cwd, func(d string) {
			if ctx.Err() != nil {
				return
			}
			select {
			case delta <- aiDelta{gen: gen, text: d}:
			case <-ctx.Done():
			}
		})
		done <- aiResult{gen: gen, text: full, err: err}
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

// aiFinish applies a completed request's result. The caller has already
// checked the generation, so this result belongs to the visible transcript.
func (s *Session) aiFinish(res aiResult) {
	s.aiCancel = nil
	o := s.ov
	if o == nil {
		return
	}
	o.streaming = false
	if res.err != nil && res.text == "" {
		o.chat = append(o.chat, chatLine{role: "note", text: res.err.Error()})
		return
	}
	// The streamed deltas already populated the transcript; extract any
	// proposed commands and strip REMEMBER lines from display.
	if len(o.chat) > 0 && o.chat[len(o.chat)-1].role == "yarp" {
		o.chat[len(o.chat)-1].text = stripRememberLines(res.text)
	}
	if res.err != nil {
		o.chat = append(o.chat, chatLine{role: "note", text: "stream ended early: " + res.err.Error()})
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
