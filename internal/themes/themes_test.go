package themes

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// Warp-format theme YAML (schema from the Rust integration fixtures).
const warpFormatTheme = `accent: '#01a0e4'
background: '#090300'
details: darker
foreground: '#a5a2a2'
terminal_colors:
  bright:
    black: '#5c5855'
    blue: '#807d7c'
    cyan: '#cdab53'
    green: '#3a3432'
    magenta: '#d6d5d4'
    red: '#e8bbd0'
    white: '#f7f7f7'
    yellow: '#4a4543'
  normal:
    black: '#090300'
    blue: '#01a0e4'
    cyan: '#b5e4f4'
    green: '#01a252'
    magenta: '#a16a94'
    red: '#db2d20'
    white: '#a5a2a2'
    yellow: '#fded02'
`

func TestLoadWarpFormatTheme(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "custom.yaml"), []byte(warpFormatTheme), 0o600)

	found, ok := Find(dir, "custom")
	if !ok {
		t.Fatal("user theme not loaded")
	}
	if found.Background != "#090300" || found.TerminalColors.Normal.Blue != "#01a0e4" {
		t.Fatalf("parsed: %+v", found)
	}
}

func TestOSCOutput(t *testing.T) {
	th, ok := Find(t.TempDir(), "yarp-dark")
	if !ok {
		t.Fatal("builtin missing")
	}
	osc := th.OSC()
	for _, want := range []string{
		"\x1b]10;#c9d1d9\x07",   // foreground
		"\x1b]11;#0d1117\x07",   // background
		"\x1b]4;0;#161b22\x07",  // normal black
		"\x1b]4;15;#f0f6fc\x07", // bright white
	} {
		if !strings.Contains(osc, want) {
			t.Errorf("OSC missing %q", want)
		}
	}
}

func TestBuiltinsPresent(t *testing.T) {
	all := Load(t.TempDir())
	if len(all) < 3 {
		t.Fatalf("expected builtins, got %d", len(all))
	}
}
