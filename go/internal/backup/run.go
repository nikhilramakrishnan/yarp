package backup

import (
	"context"
	"fmt"
	"path/filepath"
	"strings"
	"time"
)

// RunConfig captures the resolved backup settings (see config.BackupSettings).
type RunConfig struct {
	Dir          string
	RcloneRemote string
	Keep         int
	Drive        DriveAuth
	DriveFolder  string
}

// Run snapshots homeDir and ships the archive to every configured
// destination. It returns a human-readable summary of what happened; local
// snapshotting must succeed, remote targets each report independently.
func Run(homeDir string, cfg RunConfig, now time.Time) (string, error) {
	localDir := filepath.Join(homeDir, "backups")
	archive, err := Snapshot(homeDir, localDir, now)
	if err != nil {
		return "", fmt.Errorf("creating snapshot: %w", err)
	}
	if cfg.Keep > 0 {
		_, _ = Prune(localDir, cfg.Keep)
	}
	notes := []string{archive}

	if cfg.Dir != "" {
		if dst, err := CopyTo(archive, cfg.Dir); err != nil {
			notes = append(notes, "copy to "+cfg.Dir+" failed: "+err.Error())
		} else {
			notes = append(notes, dst)
		}
	}
	if cfg.RcloneRemote != "" {
		if err := UploadViaRclone(archive, cfg.RcloneRemote); err != nil {
			notes = append(notes, "rclone: "+err.Error())
		} else {
			notes = append(notes, cfg.RcloneRemote)
		}
	}
	if cfg.Drive.RefreshToken != "" {
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Minute)
		defer cancel()
		if id, err := UploadToDrive(ctx, cfg.Drive, archive, cfg.DriveFolder); err != nil {
			notes = append(notes, "google drive: "+err.Error())
		} else {
			notes = append(notes, "google drive file "+id)
		}
	}
	return strings.Join(notes, " · "), nil
}
