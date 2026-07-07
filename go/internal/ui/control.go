package ui

import (
	"path/filepath"
	"strings"
	"time"

	"github.com/nikhilramakrishnan/yarp/go/internal/blocks"
	"github.com/nikhilramakrishnan/yarp/go/internal/themes"
	"github.com/nikhilramakrishnan/yarp/go/internal/workflows"
)

// Overlay lifecycle and key handling. All of it runs on the session's main
// loop.

const (
	actionAI     = "ai"
	actionBlocks = "blocks"
	actionThemes = "themes"
	actionBackup = "backup"
)

func (s *Session) openOverlay() {
	s.ov = newOverlay(s.paletteItems())
	s.dirty = false
	s.out.WriteString(altScreenOn)
	s.repaint()
}

func (s *Session) closeOverlay() {
	if s.aiCancel != nil {
		s.aiCancel()
		s.aiCancel = nil
	}
	s.ov = nil
	s.out.WriteString(altScreenOff)
	if s.ptyDropped {
		s.ptyDropped = false
		s.out.WriteString(sgrReset + "\r\n" + sgrDim +
			"[yarp: output skipped while the overlay was open]" + sgrReset + "\r\n")
	} else if s.ptyBuf.Len() > 0 {
		s.out.Write(s.ptyBuf.Bytes())
		s.ptyBuf.Reset()
	}
	if s.dirty {
		// Nudge full-screen child apps to repaint what we may have covered.
		s.pty.Resize(s.cols, s.rows-1)
		s.pty.Resize(s.cols, s.rows)
	}
}

func (s *Session) repaint() {
	if s.ov != nil {
		s.out.WriteString(s.ov.render(s.cols, s.rows))
	}
}

// paletteItems assembles actions, workflows, and history commands.
func (s *Session) paletteItems() []paletteItem {
	items := []paletteItem{
		{label: "Ask yarp", detail: "local AI, session-aware", kind: itemAction, action: actionAI},
		{label: "Blocks", detail: "browse command output", kind: itemAction, action: actionBlocks},
		{label: "Themes", detail: "restyle this terminal", kind: itemAction, action: actionThemes},
		{label: "Back up now", detail: "snapshot ~/.yarp", kind: itemAction, action: actionBackup},
	}
	wfs, _ := workflows.Load(filepath.Join(s.homeDir, "workflows"))
	for i := range wfs {
		w := wfs[i]
		items = append(items, paletteItem{
			label: w.Name, detail: w.Description, kind: itemWorkflow, cmd: w.Command, wf: &w,
		})
	}
	historyDir := filepath.Join(s.homeDir, "history")
	cmds, _ := blocks.Commands(historyDir, 200)
	for _, c := range cmds {
		items = append(items, paletteItem{label: c, kind: itemCommand, cmd: c})
	}
	return items
}

// insertIntoShell types text at the shell prompt. Newlines are flattened so
// nothing can auto-execute: the user always presses Enter themselves.
func (s *Session) insertIntoShell(cmd string) {
	cmd = strings.TrimSpace(strings.ReplaceAll(cmd, "\n", " "))
	s.closeOverlay()
	s.pty.Write([]byte(cmd))
}

func (s *Session) handleOverlayKey(k Key) {
	o := s.ov
	if k.Kind == KeyCtrl && k.Ctrl == s.settings.PaletteByte() {
		s.closeOverlay()
		return
	}
	switch o.mode {
	case modePalette:
		s.keyPalette(k)
	case modeBlocks:
		s.keyBlocks(k)
	case modeBlockDetail:
		s.keyDetail(k)
	case modeAI:
		s.keyAI(k)
	case modeThemes:
		s.keyThemes(k)
	case modeWorkflowArgs:
		s.keyArgs(k)
	}
}

func (s *Session) keyPalette(k Key) {
	o := s.ov
	switch k.Kind {
	case KeyEsc:
		s.closeOverlay()
	case KeyUp:
		o.moveSel(-1, len(o.view))
	case KeyDown, KeyTab:
		o.moveSel(1, len(o.view))
	case KeyBackspace:
		if o.query != "" {
			r := []rune(o.query)
			o.query = string(r[:len(r)-1])
			o.filter()
		}
	case KeyRune:
		o.query += string(k.Rune)
		o.filter()
	case KeyEnter:
		if o.sel >= len(o.view) {
			return
		}
		it := o.view[o.sel]
		switch it.kind {
		case itemCommand:
			s.insertIntoShell(it.cmd)
		case itemWorkflow:
			if len(it.wf.Arguments) == 0 {
				s.insertIntoShell(it.wf.Command)
				return
			}
			o.mode = modeWorkflowArgs
			o.wf = it.wf
			o.argIdx = 0
			o.argVals = map[string]string{}
			o.argInput = it.wf.Arguments[0].DefaultValue
		case itemAction:
			s.runAction(it.action)
		}
	}
}

func (s *Session) runAction(action string) {
	o := s.ov
	switch action {
	case actionAI:
		o.mode = modeAI
	case actionBlocks:
		o.blockList = s.recentBlocks(200)
		o.blockSel = 0
		o.mode = modeBlocks
	case actionThemes:
		o.themeList = themes.Load(filepath.Join(s.homeDir, "themes"))
		o.themeSel = 0
		o.mode = modeThemes
	case actionBackup:
		s.backupNow()
	}
}

// recentBlocks merges this session's blocks with persisted history.
func (s *Session) recentBlocks(limit int) []blocks.Block {
	out := make([]blocks.Block, 0, limit)
	for i := len(s.recent) - 1; i >= 0 && len(out) < limit; i-- {
		out = append(out, s.recent[i])
	}
	if len(out) < limit {
		hist, _ := blocks.LoadRecent(filepath.Join(s.homeDir, "history"), limit)
		for _, b := range hist {
			if b.Session == s.store.Session() {
				continue // already included from memory
			}
			out = append(out, b)
			if len(out) == limit {
				break
			}
		}
	}
	return out
}

func (s *Session) keyBlocks(k Key) {
	o := s.ov
	switch k.Kind {
	case KeyEsc:
		o.mode = modePalette
	case KeyUp:
		o.moveSel2(&o.blockSel, -1, len(o.blockList))
	case KeyDown:
		o.moveSel2(&o.blockSel, 1, len(o.blockList))
	case KeyEnter:
		if o.blockSel < len(o.blockList) {
			o.detail = &o.blockList[o.blockSel]
			o.detailTop = 0
			o.mode = modeBlockDetail
		}
	case KeyRune:
		if k.Rune == 'i' && o.blockSel < len(o.blockList) {
			s.insertIntoShell(o.blockList[o.blockSel].Cmd)
		}
	}
}

func (s *Session) keyDetail(k Key) {
	o := s.ov
	switch k.Kind {
	case KeyEsc:
		o.mode = modeBlocks
	case KeyUp:
		o.detailTop--
	case KeyDown:
		o.detailTop++
	case KeyPgUp:
		o.detailTop -= s.rows - 4
	case KeyPgDn:
		o.detailTop += s.rows - 4
	case KeyRune:
		switch k.Rune {
		case 'i':
			s.insertIntoShell(o.detail.Cmd)
		case 'c':
			// OSC 52: hand the command to the host terminal's clipboard.
			s.out.WriteString(osc52(o.detail.Cmd))
			o.status = "copied to clipboard"
		}
	}
}

func (s *Session) keyThemes(k Key) {
	o := s.ov
	switch k.Kind {
	case KeyEsc:
		o.mode = modePalette
	case KeyUp:
		o.moveSel2(&o.themeSel, -1, len(o.themeList))
	case KeyDown:
		o.moveSel2(&o.themeSel, 1, len(o.themeList))
	case KeyEnter:
		if o.themeSel < len(o.themeList) {
			t := o.themeList[o.themeSel]
			s.out.WriteString(t.OSC())
			s.settings.Theme = t.Name
			_ = s.settings.Save()
			o.status = "theme: " + t.Name
		}
	}
}

func (s *Session) keyArgs(k Key) {
	o := s.ov
	switch k.Kind {
	case KeyEsc:
		o.mode = modePalette
	case KeyBackspace:
		if o.argInput != "" {
			r := []rune(o.argInput)
			o.argInput = string(r[:len(r)-1])
		}
	case KeyRune:
		o.argInput += string(k.Rune)
	case KeyEnter:
		arg := o.wf.Arguments[o.argIdx]
		o.argVals[arg.Name] = o.argInput
		o.argIdx++
		if o.argIdx < len(o.wf.Arguments) {
			o.argInput = o.wf.Arguments[o.argIdx].DefaultValue
			return
		}
		s.insertIntoShell(o.wf.Fill(o.argVals))
	}
}

// moveSel2 is moveSel for cursors other than the palette selection.
func (o *overlay) moveSel2(sel *int, delta, n int) {
	if n == 0 {
		return
	}
	*sel = (*sel + delta + n) % n
}

func osc52(text string) string {
	return "\x1b]52;c;" + base64Std(text) + "\x07"
}

func (s *Session) backupNow() {
	path, err := runBackup(s.settings, s.homeDir, time.Now())
	if err != nil {
		s.ov.status = "backup failed: " + err.Error()
		return
	}
	s.ov.status = "backup written: " + path
}
