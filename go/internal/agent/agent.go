// Package agent is yarp's one and only agent: a question-answering assistant
// grounded in the current terminal session.
//
// This deliberately replaces the entire agent-management surface of the Rust
// app (app/src/ai/agent_management, cloud_agent_config, ambient_agents,
// agent_sdk, conversation orchestration). There are no fleets, no cloud
// runs, no autonomy settings. The agent sees recent blocks and remembered
// facts, answers questions, and may propose commands — which are only ever
// inserted into the prompt for the user to run, never executed by yarp.
package agent

import (
	"context"
	"fmt"
	"regexp"
	"strings"
	"time"

	"github.com/nikhilramakrishnan/yarp/go/internal/blocks"
	"github.com/nikhilramakrishnan/yarp/go/internal/llm"
)

// Agent binds a model client to session context and memory.
type Agent struct {
	Client *llm.Client
	Memory *llm.Memory // nil when memory is disabled

	history []llm.Message // in-session conversation
}

const systemPrompt = `You are yarp, a fast local terminal assistant. You run entirely on the user's machine.
Be terse and practical. Answer questions about the user's terminal session, commands, and errors.
When you propose a shell command, put it alone in a fenced code block so the user can insert it; yarp never runs commands for you.
When the user tells you something worth keeping (preferences, project facts), you may end your reply with a line "REMEMBER: <one short fact>".`

// rememberDirective extracts facts the model asked to persist.
var rememberDirective = regexp.MustCompile(`(?m)^REMEMBER:\s*(.+)$`)

// codeFence extracts proposed commands.
var codeFence = regexp.MustCompile("(?s)```(?:[a-z]*\n)?(.*?)```")

// Reply answers one user message, streaming via onDelta. recent supplies
// terminal context (newest first, as loaded from history).
func (a *Agent) Reply(ctx context.Context, userMsg string, recent []blocks.Block, cwd string, onDelta func(string)) (string, error) {
	msgs := []llm.Message{{Role: "system", Content: systemPrompt}}
	if ctxMsg := a.contextMessage(userMsg, recent, cwd); ctxMsg != "" {
		msgs = append(msgs, llm.Message{Role: "system", Content: ctxMsg})
	}
	msgs = append(msgs, a.history...)
	msgs = append(msgs, llm.Message{Role: "user", Content: userMsg})

	full, err := a.Client.Chat(ctx, msgs, onDelta)
	if err != nil {
		return "", err
	}
	a.history = append(a.history,
		llm.Message{Role: "user", Content: userMsg},
		llm.Message{Role: "assistant", Content: full},
	)
	// Keep the rolling window small; local models have small contexts.
	if len(a.history) > 12 {
		a.history = a.history[len(a.history)-12:]
	}
	a.persistDirectives(full)
	return full, nil
}

// contextMessage assembles terminal context and recalled memory.
func (a *Agent) contextMessage(userMsg string, recent []blocks.Block, cwd string) string {
	var b strings.Builder
	if cwd != "" {
		fmt.Fprintf(&b, "Current directory: %s\n", cwd)
	}
	if len(recent) > 0 {
		b.WriteString("Recent commands (newest first):\n")
		for _, blk := range recent {
			fmt.Fprintf(&b, "$ %s  (exit %d)\n", blk.Cmd, blk.ExitCode)
			out := strings.TrimSpace(blk.Output)
			if out != "" {
				if len(out) > 1200 {
					out = out[:600] + "\n…\n" + out[len(out)-600:]
				}
				b.WriteString(out + "\n")
			}
		}
	}
	if a.Memory != nil {
		if facts, _ := a.Memory.Recall(userMsg, 5, time.Now()); len(facts) > 0 {
			b.WriteString("Remembered facts:\n")
			for _, f := range facts {
				b.WriteString("- " + f.Text + "\n")
			}
		}
	}
	return strings.TrimSpace(b.String())
}

func (a *Agent) persistDirectives(reply string) {
	if a.Memory == nil {
		return
	}
	for _, m := range rememberDirective.FindAllStringSubmatch(reply, 3) {
		fact := strings.TrimSpace(m[1])
		if fact != "" {
			_ = a.Memory.Remember(fact, time.Now())
		}
	}
}

// SuggestedCommands returns commands the reply proposed in code fences,
// first line of each fence only.
func SuggestedCommands(reply string) []string {
	var out []string
	for _, m := range codeFence.FindAllStringSubmatch(reply, 5) {
		lines := strings.Split(strings.TrimSpace(m[1]), "\n")
		if len(lines) > 0 && strings.TrimSpace(lines[0]) != "" {
			out = append(out, strings.TrimSpace(lines[0]))
		}
	}
	return out
}
