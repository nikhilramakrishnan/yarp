// Package themes loads Warp-format theme YAML and applies it to the host
// terminal with standard OSC color sequences (OSC 10/11/12 and OSC 4).
//
// Because yarp renders through the user's own terminal, "applying a theme"
// means reprogramming that terminal's palette — no GPU renderer required.
// Any theme from Warp's public theme repository drops into ~/.yarp/themes.
package themes

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"gopkg.in/yaml.v3"
)

// Theme mirrors the Warp theme YAML schema
// (see crates/integration/tests/data/test_theme.yaml in the Rust tree).
type Theme struct {
	Name           string         `yaml:"name,omitempty"`
	Accent         string         `yaml:"accent"`
	Background     string         `yaml:"background"`
	Foreground     string         `yaml:"foreground"`
	Details        string         `yaml:"details,omitempty"` // "darker" | "lighter"
	TerminalColors TerminalColors `yaml:"terminal_colors"`
}

// TerminalColors is the 16-color ANSI palette.
type TerminalColors struct {
	Normal Palette `yaml:"normal"`
	Bright Palette `yaml:"bright"`
}

// Palette names the eight base slots.
type Palette struct {
	Black   string `yaml:"black"`
	Red     string `yaml:"red"`
	Green   string `yaml:"green"`
	Yellow  string `yaml:"yellow"`
	Blue    string `yaml:"blue"`
	Magenta string `yaml:"magenta"`
	Cyan    string `yaml:"cyan"`
	White   string `yaml:"white"`
}

func (p Palette) slots() []string {
	return []string{p.Black, p.Red, p.Green, p.Yellow, p.Blue, p.Magenta, p.Cyan, p.White}
}

// OSC renders the escape sequences that apply the theme to the host terminal.
func (t *Theme) OSC() string {
	var b strings.Builder
	if t.Foreground != "" {
		fmt.Fprintf(&b, "\x1b]10;%s\x07", t.Foreground)
	}
	if t.Background != "" {
		fmt.Fprintf(&b, "\x1b]11;%s\x07", t.Background)
	}
	if t.Accent != "" {
		fmt.Fprintf(&b, "\x1b]12;%s\x07", t.Accent) // cursor color
	}
	for i, c := range t.TerminalColors.Normal.slots() {
		if c != "" {
			fmt.Fprintf(&b, "\x1b]4;%d;%s\x07", i, c)
		}
	}
	for i, c := range t.TerminalColors.Bright.slots() {
		if c != "" {
			fmt.Fprintf(&b, "\x1b]4;%d;%s\x07", i+8, c)
		}
	}
	return b.String()
}

// Load returns built-in themes plus user themes from dir, sorted by name.
// User themes shadow built-ins with the same name.
func Load(dir string) []Theme {
	byName := make(map[string]Theme, len(builtin))
	for _, t := range builtin {
		byName[t.Name] = t
	}
	if entries, err := os.ReadDir(dir); err == nil {
		for _, e := range entries {
			name := e.Name()
			if e.IsDir() || (!strings.HasSuffix(name, ".yaml") && !strings.HasSuffix(name, ".yml")) {
				continue
			}
			raw, err := os.ReadFile(filepath.Join(dir, name))
			if err != nil {
				continue
			}
			var t Theme
			if yaml.Unmarshal(raw, &t) != nil || t.Background == "" {
				continue
			}
			if t.Name == "" {
				t.Name = strings.TrimSuffix(strings.TrimSuffix(name, ".yml"), ".yaml")
			}
			byName[t.Name] = t
		}
	}
	out := make([]Theme, 0, len(byName))
	for _, t := range byName {
		out = append(out, t)
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Name < out[j].Name })
	return out
}

// Find returns the named theme.
func Find(dir, name string) (Theme, bool) {
	for _, t := range Load(dir) {
		if strings.EqualFold(t.Name, name) {
			return t, true
		}
	}
	return Theme{}, false
}

// builtin ships a small tasteful set; users add more as YAML files.
var builtin = []Theme{
	{
		Name: "yarp-dark", Accent: "#01a0e4", Background: "#0d1117", Foreground: "#c9d1d9", Details: "darker",
		TerminalColors: TerminalColors{
			Normal: Palette{Black: "#161b22", Red: "#f85149", Green: "#3fb950", Yellow: "#d29922", Blue: "#58a6ff", Magenta: "#bc8cff", Cyan: "#39c5cf", White: "#b1bac4"},
			Bright: Palette{Black: "#484f58", Red: "#ff7b72", Green: "#56d364", Yellow: "#e3b341", Blue: "#79c0ff", Magenta: "#d2a8ff", Cyan: "#56d4dd", White: "#f0f6fc"},
		},
	},
	{
		Name: "yarp-light", Accent: "#0969da", Background: "#ffffff", Foreground: "#1f2328", Details: "lighter",
		TerminalColors: TerminalColors{
			Normal: Palette{Black: "#24292f", Red: "#cf222e", Green: "#116329", Yellow: "#4d2d00", Blue: "#0969da", Magenta: "#8250df", Cyan: "#1b7c83", White: "#6e7781"},
			Bright: Palette{Black: "#57606a", Red: "#a40e26", Green: "#1a7f37", Yellow: "#633c01", Blue: "#218bff", Magenta: "#a475f9", Cyan: "#3192aa", White: "#8c959f"},
		},
	},
	{
		Name: "hot-fuzz", Accent: "#fded02", Background: "#090300", Foreground: "#a5a2a2", Details: "darker",
		TerminalColors: TerminalColors{
			Normal: Palette{Black: "#090300", Red: "#db2d20", Green: "#01a252", Yellow: "#fded02", Blue: "#01a0e4", Magenta: "#a16a94", Cyan: "#b5e4f4", White: "#a5a2a2"},
			Bright: Palette{Black: "#5c5855", Red: "#e8bbd0", Green: "#3a3432", Yellow: "#4a4543", Blue: "#807d7c", Magenta: "#d6d5d4", Cyan: "#cdab53", White: "#f7f7f7"},
		},
	},
}
