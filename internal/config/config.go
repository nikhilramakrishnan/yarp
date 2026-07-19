// Package config owns the ~/.yarp data directory and settings.json.
//
// Everything yarp knows about the user lives under one directory that the
// user can read, edit, delete, and back up with ordinary tools. There is no
// account, no hosted sync, and no telemetry.
package config

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
)

// Settings is the on-disk settings.json schema. Zero values fall back to
// defaults at read time so a hand-edited partial file is always valid.
type Settings struct {
	// Shell overrides the login shell yarp launches. Empty = $SHELL (unix)
	// or pwsh/powershell/cmd discovery (windows).
	Shell string `json:"shell,omitempty"`
	// Theme is the name of the active theme (built-in or ~/.yarp/themes/*.yaml).
	Theme string `json:"theme,omitempty"`
	// PaletteKey is the control byte that opens the overlay. Stored as a
	// printable mnemonic like "ctrl-g".
	PaletteKey string `json:"palette_key,omitempty"`
	// BlockOutputLimitKB caps how much plain-text output is persisted per block.
	BlockOutputLimitKB int `json:"block_output_limit_kb,omitempty"`
	// HistoryDays is how long session history is retained before pruning.
	HistoryDays int `json:"history_days,omitempty"`

	LLM    LLMSettings    `json:"llm"`
	Backup BackupSettings `json:"backup"`
}

// LLMSettings configures local model access. Only local, OpenAI-compatible
// runtimes are supported; there is deliberately no field for a hosted API key.
type LLMSettings struct {
	// Endpoint is an OpenAI-compatible base URL, e.g. http://127.0.0.1:11434/v1.
	// Empty = auto-discover well-known local runtimes.
	Endpoint string `json:"endpoint,omitempty"`
	// Model pins a model name. Empty = first model the runtime reports.
	Model string `json:"model,omitempty"`
	// MemoryEnabled persists conversational memory to ~/.yarp/memory.jsonl.
	MemoryEnabled *bool `json:"memory_enabled,omitempty"`
	// ContextBlocks is how many recent blocks are shared with the model.
	ContextBlocks int `json:"context_blocks,omitempty"`
}

// BackupSettings configures personal backup targets.
type BackupSettings struct {
	// Dir is a local/removable/NAS path snapshots are copied to.
	Dir string `json:"dir,omitempty"`
	// RcloneRemote is an rclone destination like "gdrive:yarp-backups".
	RcloneRemote string `json:"rclone_remote,omitempty"`
	// Keep is how many snapshots to retain locally.
	Keep int `json:"keep,omitempty"`

	GoogleDrive GoogleDriveSettings `json:"google_drive"`
}

// GoogleDriveSettings holds the user's own OAuth client and grant. The user
// creates the OAuth client in their own Google Cloud project; yarp ships no
// credentials and talks to nothing but the user's Drive.
type GoogleDriveSettings struct {
	ClientID     string `json:"client_id,omitempty"`
	ClientSecret string `json:"client_secret,omitempty"`
	RefreshToken string `json:"refresh_token,omitempty"`
	FolderID     string `json:"folder_id,omitempty"`
}

const (
	defaultPaletteKey   = "ctrl-g"
	defaultOutputKB     = 256
	defaultHistoryDays  = 90
	defaultCtxBlocks    = 8
	defaultBackupKeep   = 10
	settingsFileName    = "settings.json"
	dirPermissions      = 0o700
	settingsPermissions = 0o600
)

// Dir returns the yarp home directory, creating it if needed.
// Override with $YARP_HOME (used heavily by tests).
func Dir() (string, error) {
	if d := os.Getenv("YARP_HOME"); d != "" {
		return d, os.MkdirAll(d, dirPermissions)
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return "", fmt.Errorf("resolving home directory: %w", err)
	}
	d := filepath.Join(home, ".yarp")
	return d, os.MkdirAll(d, dirPermissions)
}

// MemoryPath is the single definition of where agent memory lives; the
// overlay and every CLI subcommand must agree on it.
func MemoryPath() (string, error) {
	base, err := Dir()
	if err != nil {
		return "", err
	}
	return filepath.Join(base, "memory.jsonl"), nil
}

// Subdir returns (and creates) a directory under the yarp home.
func Subdir(name string) (string, error) {
	base, err := Dir()
	if err != nil {
		return "", err
	}
	d := filepath.Join(base, name)
	return d, os.MkdirAll(d, dirPermissions)
}

// Load reads settings.json, filling defaults for anything unset. A missing
// file is not an error: first run works with zero configuration.
func Load() (*Settings, error) {
	base, err := Dir()
	if err != nil {
		return nil, err
	}
	s := &Settings{}
	raw, err := os.ReadFile(filepath.Join(base, settingsFileName))
	switch {
	case errors.Is(err, os.ErrNotExist):
		// defaults only
	case err != nil:
		return nil, fmt.Errorf("reading settings: %w", err)
	default:
		if err := json.Unmarshal(raw, s); err != nil {
			return nil, fmt.Errorf("settings.json is not valid JSON: %w", err)
		}
	}
	s.applyDefaults()
	return s, nil
}

// Save writes settings.json atomically.
func (s *Settings) Save() error {
	base, err := Dir()
	if err != nil {
		return err
	}
	raw, err := json.MarshalIndent(s, "", "  ")
	if err != nil {
		return err
	}
	tmp := filepath.Join(base, settingsFileName+".tmp")
	if err := os.WriteFile(tmp, append(raw, '\n'), settingsPermissions); err != nil {
		return err
	}
	return os.Rename(tmp, filepath.Join(base, settingsFileName))
}

func (s *Settings) applyDefaults() {
	if s.PaletteKey == "" {
		s.PaletteKey = defaultPaletteKey
	}
	if s.BlockOutputLimitKB <= 0 {
		s.BlockOutputLimitKB = defaultOutputKB
	}
	if s.HistoryDays <= 0 {
		s.HistoryDays = defaultHistoryDays
	}
	if s.LLM.ContextBlocks <= 0 {
		s.LLM.ContextBlocks = defaultCtxBlocks
	}
	if s.Backup.Keep <= 0 {
		s.Backup.Keep = defaultBackupKeep
	}
}

// MemoryEnabled reports whether agent memory is on (default true).
func (s *Settings) MemoryOn() bool {
	return s.LLM.MemoryEnabled == nil || *s.LLM.MemoryEnabled
}

// PaletteByte maps the configured palette key mnemonic to its control byte.
func (s *Settings) PaletteByte() byte {
	name := s.PaletteKey
	if len(name) == len("ctrl-x") && name[:5] == "ctrl-" {
		c := name[5]
		if c >= 'a' && c <= 'z' {
			return c - 'a' + 1
		}
	}
	return 0x07 // ctrl-g
}
