package workflows

import (
	"os"
	"path/filepath"
	"testing"
)

// The YAML here matches the Warp workflow format verbatim (same schema the
// Rust app loaded), proving existing workflow assets keep working.
const warpFormatWorkflow = `name: "Run Yarp locally with shell"
command: "YARP_SHELL_PATH={{shell}} cargo run"
description: "Runs yarp with the particular shell"
arguments:
  - name: shell
    description: path of shell to use
    default_value: /bin/zsh
author: Yarp Team
shells: []
`

func TestLoadWarpFormat(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "run.yaml"), []byte(warpFormatWorkflow), 0o600)
	os.WriteFile(filepath.Join(dir, "broken.yaml"), []byte("::::"), 0o600)

	wfs, err := Load(dir)
	if err != nil {
		t.Fatal(err)
	}
	if len(wfs) != 1 {
		t.Fatalf("got %d workflows (broken file must be skipped)", len(wfs))
	}
	w := wfs[0]
	if w.Name != "Run Yarp locally with shell" || len(w.Arguments) != 1 {
		t.Fatalf("parsed: %+v", w)
	}
}

func TestFill(t *testing.T) {
	w := Workflow{
		Command: "echo {{a}} {{b}} {{missing}}",
		Arguments: []Argument{
			{Name: "a"},
			{Name: "b", DefaultValue: "beta"},
		},
	}
	got := w.Fill(map[string]string{"a": "alpha"})
	if got != "echo alpha beta {{missing}}" {
		t.Fatalf("Fill = %q", got)
	}
}

func TestLoadMissingDir(t *testing.T) {
	wfs, err := Load("/nonexistent/workflows")
	if err != nil || wfs != nil {
		t.Fatalf("missing dir: %v %v", wfs, err)
	}
}
