// Package backup snapshots the ~/.yarp directory into portable tar.gz
// archives and ships them to personally-owned destinations.
//
// This replaces the hosted object sync (cloud_object/, drive/, sync_queue in
// the Rust app) with the opposite philosophy: your data is a directory, a
// backup is a file, and the destinations are things you own — another disk,
// your own Google Drive, or any rclone remote.
package backup

import (
	"archive/tar"
	"compress/gzip"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

const snapshotPrefix = "yarp-backup-"

// zeroReader pads tar entries whose file shrank mid-snapshot.
type zeroReader struct{}

func (zeroReader) Read(p []byte) (int, error) {
	for i := range p {
		p[i] = 0
	}
	return len(p), nil
}

// Snapshot writes a tar.gz of srcDir (the yarp home) into destDir and
// returns the archive path. Runtime scratch and the backups directory itself
// are excluded.
func Snapshot(srcDir, destDir string, now time.Time) (string, error) {
	if err := os.MkdirAll(destDir, 0o700); err != nil {
		return "", err
	}
	name := snapshotPrefix + now.Format("20060102-150405") + ".tar.gz"
	path := filepath.Join(destDir, name)
	f, err := os.Create(path)
	if err != nil {
		return "", err
	}
	defer f.Close()
	gz := gzip.NewWriter(f)
	tw := tar.NewWriter(gz)

	skip := map[string]bool{"backups": true, "runtime": true}
	err = filepath.Walk(srcDir, func(p string, info os.FileInfo, err error) error {
		if err != nil {
			return nil // unreadable entries shouldn't kill the backup
		}
		rel, err := filepath.Rel(srcDir, p)
		if err != nil || rel == "." {
			return nil
		}
		rel = filepath.ToSlash(rel)
		top, _, _ := strings.Cut(rel, "/")
		if skip[top] {
			if info.IsDir() {
				return filepath.SkipDir
			}
			return nil
		}
		if !info.IsDir() && !info.Mode().IsRegular() {
			return nil // symlinks, sockets: nothing worth archiving
		}
		hdr, err := tar.FileInfoHeader(info, "")
		if err != nil {
			return err
		}
		hdr.Name = rel
		if info.IsDir() {
			hdr.Name += "/"
			return tw.WriteHeader(hdr)
		}
		// Open BEFORE committing the header: writing a header and then
		// failing to supply its bytes corrupts the whole archive, whereas
		// skipping an unreadable file just narrows the backup.
		src, err := os.Open(p)
		if err != nil {
			return nil
		}
		defer src.Close()
		if err := tw.WriteHeader(hdr); err != nil {
			return err
		}
		// The file may change size between stat and copy (another yarp
		// session appending history): copy exactly the declared size and
		// zero-pad if it shrank, so one racing file can't abort the backup.
		n, err := io.CopyN(tw, src, hdr.Size)
		if err == io.EOF {
			_, err = io.CopyN(tw, zeroReader{}, hdr.Size-n)
		}
		return err
	})
	if err != nil {
		return "", err
	}
	if err := tw.Close(); err != nil {
		return "", err
	}
	if err := gz.Close(); err != nil {
		return "", err
	}
	return path, f.Close()
}

// Prune keeps the newest `keep` snapshots in dir and deletes the rest.
func Prune(dir string, keep int) (int, error) {
	entries, err := os.ReadDir(dir)
	if os.IsNotExist(err) {
		return 0, nil
	}
	if err != nil {
		return 0, err
	}
	var names []string
	for _, e := range entries {
		if !e.IsDir() && strings.HasPrefix(e.Name(), snapshotPrefix) && strings.HasSuffix(e.Name(), ".tar.gz") {
			names = append(names, e.Name())
		}
	}
	sort.Strings(names) // timestamped names sort chronologically
	removed := 0
	for len(names) > keep {
		if os.Remove(filepath.Join(dir, names[0])) == nil {
			removed++
		}
		names = names[1:]
	}
	return removed, nil
}

// Restore unpacks an archive into destDir, refusing entries that would
// escape it.
func Restore(archive, destDir string) error {
	f, err := os.Open(archive)
	if err != nil {
		return err
	}
	defer f.Close()
	gz, err := gzip.NewReader(f)
	if err != nil {
		return err
	}
	defer gz.Close()
	tr := tar.NewReader(gz)
	for {
		hdr, err := tr.Next()
		if err == io.EOF {
			return nil
		}
		if err != nil {
			return err
		}
		target := filepath.Join(destDir, filepath.FromSlash(hdr.Name))
		rel, err := filepath.Rel(destDir, target)
		if err != nil || strings.HasPrefix(rel, "..") {
			return fmt.Errorf("archive entry escapes destination: %s", hdr.Name)
		}
		switch hdr.Typeflag {
		case tar.TypeDir:
			if err := os.MkdirAll(target, 0o700); err != nil {
				return err
			}
		case tar.TypeReg:
			if err := os.MkdirAll(filepath.Dir(target), 0o700); err != nil {
				return err
			}
			out, err := os.OpenFile(target, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, os.FileMode(hdr.Mode)&0o777)
			if err != nil {
				return err
			}
			if _, err := io.Copy(out, tr); err != nil {
				out.Close()
				return err
			}
			if err := out.Close(); err != nil {
				return err
			}
		}
	}
}

// CopyTo copies an archive to a destination directory (external disk, NAS
// mount — anything that looks like a path).
func CopyTo(archive, destDir string) (string, error) {
	if err := os.MkdirAll(destDir, 0o700); err != nil {
		return "", err
	}
	dst := filepath.Join(destDir, filepath.Base(archive))
	in, err := os.Open(archive)
	if err != nil {
		return "", err
	}
	defer in.Close()
	out, err := os.Create(dst)
	if err != nil {
		return "", err
	}
	if _, err := io.Copy(out, in); err != nil {
		out.Close()
		return "", err
	}
	return dst, out.Close()
}
