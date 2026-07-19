// Package termio abstracts the platform PTY: Unix ptys via creack/pty and
// Windows ConPTY. Everything above this package is platform-independent,
// which is what keeps the single binary cross-platform.
package termio

import (
	"io"
	"os/exec"
)

// Pty is a running child process attached to a pseudo-terminal.
type Pty interface {
	io.ReadWriter
	// Resize updates the child's window size.
	Resize(cols, rows int) error
	// Wait blocks until the child exits and returns its exit code.
	Wait() (int, error)
	// Close releases the pty. Safe to call after Wait.
	Close() error
}

// Command describes the child to launch.
type Command struct {
	Path string
	Args []string
	Env  []string
	Dir  string
}

func (c Command) execCmd() *exec.Cmd {
	cmd := exec.Command(c.Path, c.Args...)
	cmd.Env = c.Env
	cmd.Dir = c.Dir
	return cmd
}
