//go:build windows

package ui

import "os/exec"

func defaultShell() string {
	for _, sh := range []string{"pwsh.exe", "powershell.exe"} {
		if p, err := exec.LookPath(sh); err == nil {
			return p
		}
	}
	return "cmd.exe"
}
