// Package workflows reads Warp-format workflow YAML files. The format is
// unchanged from the Rust app (app/src/workflows/local_workflows.rs), so
// existing .yaml workflows — including the repo's own .warp/workflows — load
// as-is from ~/.yarp/workflows.
package workflows

import (
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"

	"gopkg.in/yaml.v3"
)

// Workflow is one parameterized command template.
type Workflow struct {
	Name        string     `yaml:"name"`
	Command     string     `yaml:"command"`
	Description string     `yaml:"description,omitempty"`
	Arguments   []Argument `yaml:"arguments,omitempty"`
	Author      string     `yaml:"author,omitempty"`
	Shells      []string   `yaml:"shells,omitempty"`
}

// Argument is a {{placeholder}} in the command.
type Argument struct {
	Name         string `yaml:"name"`
	Description  string `yaml:"description,omitempty"`
	DefaultValue string `yaml:"default_value,omitempty"`
}

var placeholder = regexp.MustCompile(`\{\{\s*([A-Za-z0-9_.-]+)\s*\}\}`)

// Fill substitutes argument values into the command. Missing values fall back
// to the argument's default, then to the raw placeholder so the user can see
// what's unfilled.
func (w *Workflow) Fill(values map[string]string) string {
	defaults := make(map[string]string, len(w.Arguments))
	for _, a := range w.Arguments {
		defaults[a.Name] = a.DefaultValue
	}
	return placeholder.ReplaceAllStringFunc(w.Command, func(m string) string {
		name := placeholder.FindStringSubmatch(m)[1]
		if v, ok := values[name]; ok && v != "" {
			return v
		}
		if v := defaults[name]; v != "" {
			return v
		}
		return m
	})
}

// Load reads every *.yaml/*.yml workflow under dir, sorted by name.
// A missing directory yields an empty list.
func Load(dir string) ([]Workflow, error) {
	entries, err := os.ReadDir(dir)
	if os.IsNotExist(err) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	var out []Workflow
	for _, e := range entries {
		name := e.Name()
		if e.IsDir() || (!strings.HasSuffix(name, ".yaml") && !strings.HasSuffix(name, ".yml")) {
			continue
		}
		raw, err := os.ReadFile(filepath.Join(dir, name))
		if err != nil {
			continue
		}
		var w Workflow
		if yaml.Unmarshal(raw, &w) != nil || w.Name == "" || w.Command == "" {
			continue // one bad file must not hide the rest
		}
		out = append(out, w)
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Name < out[j].Name })
	return out, nil
}
