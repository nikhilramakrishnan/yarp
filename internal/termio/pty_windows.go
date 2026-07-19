//go:build windows

package termio

import (
	"context"
	"strings"

	"github.com/UserExistsError/conpty"
)

type winPty struct {
	cpty *conpty.ConPty
}

// Start launches the command attached to a ConPTY of the given size.
func Start(c Command, cols, rows int) (Pty, error) {
	parts := append([]string{quoteArg(c.Path)}, quoteArgs(c.Args)...)
	opts := []conpty.ConPtyOption{conpty.ConPtyDimensions(cols, rows)}
	if c.Dir != "" {
		opts = append(opts, conpty.ConPtyWorkDir(c.Dir))
	}
	if len(c.Env) > 0 {
		opts = append(opts, conpty.ConPtyEnv(c.Env))
	}
	cp, err := conpty.Start(strings.Join(parts, " "), opts...)
	if err != nil {
		return nil, err
	}
	return &winPty{cpty: cp}, nil
}

func (p *winPty) Read(b []byte) (int, error)  { return p.cpty.Read(b) }
func (p *winPty) Write(b []byte) (int, error) { return p.cpty.Write(b) }

func (p *winPty) Resize(cols, rows int) error {
	return p.cpty.Resize(cols, rows)
}

func (p *winPty) Wait() (int, error) {
	code, err := p.cpty.Wait(context.Background())
	return int(code), err
}

func (p *winPty) Close() error { return p.cpty.Close() }

func quoteArgs(args []string) []string {
	out := make([]string, len(args))
	for i, a := range args {
		out[i] = quoteArg(a)
	}
	return out
}

func quoteArg(a string) string {
	if a == "" || strings.ContainsAny(a, " \t\"") {
		return `"` + strings.ReplaceAll(a, `"`, `\"`) + `"`
	}
	return a
}
