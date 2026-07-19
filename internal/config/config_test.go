package config

import (
	"os"
	"path/filepath"
	"testing"
)

func TestDefaultsWithoutFile(t *testing.T) {
	t.Setenv("YARP_HOME", t.TempDir())
	s, err := Load()
	if err != nil {
		t.Fatal(err)
	}
	if s.PaletteKey != "ctrl-g" || s.BlockOutputLimitKB != 256 || s.HistoryDays != 90 {
		t.Fatalf("defaults: %+v", s)
	}
	if !s.MemoryOn() {
		t.Fatal("memory should default on")
	}
	if s.PaletteByte() != 0x07 {
		t.Fatalf("palette byte = %#x", s.PaletteByte())
	}
}

func TestSaveLoadRoundTrip(t *testing.T) {
	t.Setenv("YARP_HOME", t.TempDir())
	s, _ := Load()
	s.Theme = "hot-fuzz"
	s.PaletteKey = "ctrl-p"
	s.Backup.RcloneRemote = "gdrive:yarp"
	if err := s.Save(); err != nil {
		t.Fatal(err)
	}
	loaded, err := Load()
	if err != nil {
		t.Fatal(err)
	}
	if loaded.Theme != "hot-fuzz" || loaded.Backup.RcloneRemote != "gdrive:yarp" {
		t.Fatalf("round trip: %+v", loaded)
	}
	if loaded.PaletteByte() != 0x10 {
		t.Fatalf("ctrl-p byte = %#x", loaded.PaletteByte())
	}
}

func TestPartialFileGetsDefaults(t *testing.T) {
	home := t.TempDir()
	t.Setenv("YARP_HOME", home)
	os.WriteFile(filepath.Join(home, "settings.json"), []byte(`{"theme":"x"}`), 0o600)
	s, err := Load()
	if err != nil {
		t.Fatal(err)
	}
	if s.Theme != "x" || s.HistoryDays != 90 {
		t.Fatalf("partial: %+v", s)
	}
}
