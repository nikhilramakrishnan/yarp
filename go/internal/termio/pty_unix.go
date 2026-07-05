//go:build !windows

package termio

import (
	"errors"
	"os"
	"os/exec"

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

func (p *unixPty) Read(b []byte) (int, error)  { return p.master.Read(b) }
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

func (p *unixPty) Close() error { return p.master.Close() }
