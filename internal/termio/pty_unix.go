//go:build !windows

package termio

import (
	"errors"
	"io"
	"os"
	"os/exec"
	"syscall"

	"github.com/creack/pty"
)

type unixPty struct {
	master *os.File
	cmd    *exec.Cmd
}

// Start launches the command attached to a new pty of the given size.
func Start(c Command, cols, rows int) (Pty, error) {
	cmd := c.execCmd()
	master, err := pty.StartWithSize(cmd, &pty.Winsize{
		Cols: uint16(cols), Rows: uint16(rows),
	})
	if err != nil {
		return nil, err
	}
	return &unixPty{master: master, cmd: cmd}, nil
}

func (p *unixPty) Read(b []byte) (int, error) {
	n, err := p.master.Read(b)
	// BSD and Linux PTYs commonly report EIO when the slave side closes.
	// At this boundary it is the terminal equivalent of EOF, not a session
	// failure that should flash an error in the desktop client.
	if errors.Is(err, syscall.EIO) {
		return n, io.EOF
	}
	return n, err
}
func (p *unixPty) Write(b []byte) (int, error) { return p.master.Write(b) }

func (p *unixPty) Resize(cols, rows int) error {
	return pty.Setsize(p.master, &pty.Winsize{Cols: uint16(cols), Rows: uint16(rows)})
}

func (p *unixPty) Wait() (int, error) {
	err := p.cmd.Wait()
	var exitErr *exec.ExitError
	if errors.As(err, &exitErr) {
		return exitErr.ExitCode(), nil
	}
	if err != nil {
		return -1, err
	}
	return 0, nil
}

func (p *unixPty) Close() error {
	var signalErr error
	if p.cmd.Process != nil {
		signalErr = p.cmd.Process.Signal(syscall.SIGHUP)
		if errors.Is(signalErr, os.ErrProcessDone) {
			signalErr = nil
		}
	}
	return errors.Join(signalErr, p.master.Close())
}
