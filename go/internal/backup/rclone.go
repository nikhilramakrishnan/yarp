package backup

import (
	"fmt"
	"os/exec"
	"strings"
)

// UploadViaRclone copies the archive to any rclone remote ("gdrive:yarp",
// "s3:bucket/path", "sftp:host:dir", …). rclone already solves every cloud
// storage protocol; when it's installed, yarp just uses it.
func UploadViaRclone(archive, remote string) error {
	if _, err := exec.LookPath("rclone"); err != nil {
		return fmt.Errorf("rclone is not installed (https://rclone.org/install/)")
	}
	out, err := exec.Command("rclone", "copyto", archive,
		strings.TrimRight(remote, "/")+"/"+baseName(archive)).CombinedOutput()
	if err != nil {
		return fmt.Errorf("rclone copy failed: %v: %s", err, strings.TrimSpace(string(out)))
	}
	return nil
}

func baseName(p string) string {
	if i := strings.LastIndexAny(p, `/\`); i >= 0 {
		return p[i+1:]
	}
	return p
}
