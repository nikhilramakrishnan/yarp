// Package llm talks to local, OpenAI-compatible model runtimes.
//
// This is the Go successor of the Rust LocalLlmProvider
// (app/src/server/local_backend/llm_provider.rs) with the hosted providers
// removed: yarp speaks only to model servers on the user's own machine or
// network — Ollama, llama.cpp server, LM Studio, vLLM, or anything else that
// serves /v1/chat/completions. There is no API-key handling at all.
package llm

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"strings"
	"time"
)

// Message is one chat turn.
type Message struct {
	Role    string `json:"role"` // system | user | assistant
	Content string `json:"content"`
}

// Client is a connection to one local runtime.
type Client struct {
	BaseURL string // e.g. http://127.0.0.1:11434/v1
	Model   string
	http    *http.Client
}

// wellKnown lists the localhost runtimes yarp probes, in preference order.
// These match what the Rust settings UI advertised.
var wellKnown = []string{
	"http://127.0.0.1:11434/v1", // Ollama
	"http://127.0.0.1:1234/v1",  // LM Studio
	"http://127.0.0.1:8080/v1",  // llama.cpp server
}

// Discover finds a usable runtime. Resolution order matches the Rust
// local_backend: explicit endpoint (settings or $YARP_LLM_BASE_URL), then
// well-known localhost ports. Returns a descriptive error when nothing is
// listening so the UI can show setup guidance instead of a stack trace.
func Discover(ctx context.Context, endpoint, model string) (*Client, error) {
	if env := os.Getenv("YARP_LLM_BASE_URL"); env != "" {
		endpoint = env
	}
	if env := os.Getenv("YARP_LLM_MODEL"); env != "" {
		model = env
	}
	candidates := wellKnown
	if endpoint != "" {
		candidates = []string{strings.TrimRight(endpoint, "/")}
	}
	hc := &http.Client{Timeout: 90 * time.Second}
	for _, base := range candidates {
		c := &Client{BaseURL: base, Model: model, http: hc}
		probeCtx, cancel := context.WithTimeout(ctx, 1500*time.Millisecond)
		models, err := c.models(probeCtx)
		cancel()
		if err != nil {
			continue
		}
		if c.Model == "" && len(models) > 0 {
			c.Model = models[0]
		}
		if c.Model == "" {
			return nil, fmt.Errorf("runtime at %s has no models loaded (try `ollama pull <model>`)", base)
		}
		return c, nil
	}
	return nil, fmt.Errorf("no local model runtime found (looked for Ollama :11434, LM Studio :1234, llama.cpp :8080; set llm.endpoint in ~/.yarp/settings.json or $YARP_LLM_BASE_URL)")
}

func (c *Client) models(ctx context.Context) ([]string, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.BaseURL+"/models", nil)
	if err != nil {
		return nil, err
	}
	resp, err := c.http.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("GET /models: %s", resp.Status)
	}
	var body struct {
		Data []struct {
			ID string `json:"id"`
		} `json:"data"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&body); err != nil {
		return nil, err
	}
	out := make([]string, 0, len(body.Data))
	for _, m := range body.Data {
		out = append(out, m.ID)
	}
	return out, nil
}

// Chat streams a completion, invoking onDelta for each content fragment, and
// returns the full response text.
func (c *Client) Chat(ctx context.Context, msgs []Message, onDelta func(string)) (string, error) {
	payload := map[string]any{
		"model":    c.Model,
		"messages": msgs,
		"stream":   true,
	}
	raw, err := json.Marshal(payload)
	if err != nil {
		return "", err
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, c.BaseURL+"/chat/completions", bytes.NewReader(raw))
	if err != nil {
		return "", err
	}
	req.Header.Set("Content-Type", "application/json")
	resp, err := c.http.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		var msg bytes.Buffer
		_, _ = msg.ReadFrom(resp.Body)
		return "", fmt.Errorf("chat request failed: %s: %s", resp.Status, strings.TrimSpace(msg.String()))
	}

	var full strings.Builder
	sc := bufio.NewScanner(resp.Body)
	sc.Buffer(make([]byte, 0, 64*1024), 1024*1024)
	for sc.Scan() {
		line := strings.TrimSpace(sc.Text())
		if !strings.HasPrefix(line, "data:") {
			continue
		}
		data := strings.TrimSpace(strings.TrimPrefix(line, "data:"))
		if data == "[DONE]" {
			break
		}
		var chunk struct {
			Choices []struct {
				Delta struct {
					Content string `json:"content"`
				} `json:"delta"`
			} `json:"choices"`
		}
		if json.Unmarshal([]byte(data), &chunk) != nil {
			continue
		}
		for _, ch := range chunk.Choices {
			if ch.Delta.Content != "" {
				full.WriteString(ch.Delta.Content)
				if onDelta != nil {
					onDelta(ch.Delta.Content)
				}
			}
		}
	}
	if err := sc.Err(); err != nil && full.Len() == 0 {
		return "", err
	}
	return full.String(), nil
}
