package ui

import (
	"fmt"
	"strings"
	"time"

	"github.com/nikhilramakrishnan/yarp/internal/blocks"
	"github.com/nikhilramakrishnan/yarp/internal/fuzzy"
	"github.com/nikhilramakrishnan/yarp/internal/themes"
	"github.com/nikhilramakrishnan/yarp/internal/workflows"
)

// The overlay is yarp's whole chrome: one modal surface on the alternate
// screen, opened with a hotkey, closed with Esc. Everything else is the
// user's own terminal. Modes: palette (default), block list, block detail,
// AI chat, theme picker, workflow argument fill.

type overlayMode int

const (
	modePalette overlayMode = iota
	modeBlocks
	modeBlockDetail
	modeAI
	modeThemes
	modeWorkflowArgs
)

type itemKind int

const (
	itemCommand itemKind = iota
	itemWorkflow
	itemAction
)

type paletteItem struct {
	label  string
	detail string
	kind   itemKind
	cmd    string
	wf     *workflows.Workflow
	action string
	// key is the precomputed lowercase search text: fuzzy-ranking ~hundreds
	// of items per keystroke must not re-lowercase them every time.
	key string
}

type chatLine struct {
	role string // "you" | "yarp" | "note"
	text string
}

// overlay holds all modal state. It is only touched from the session's main
// loop, so it needs no locking.
type overlay struct {
	mode overlayMode

	// palette
	query string
	items []paletteItem
	view  []paletteItem
	sel   int

	// blocks
	blockList []blocks.Block
	blockSel  int
	detail    *blocks.Block
	detailTop int
	// wrapped-output cache: re-wrapping a 256KB output on every scroll
	// keystroke is the difference between instant and seconds.
	detailLines []string
	detailWidth int

	// workflow arg fill
	wf       *workflows.Workflow
	argIdx   int
	argVals  map[string]string
	argInput string

	// ai
	chat      []chatLine
	aiInput   string
	streaming bool
	suggested []string

	// themes
	themeList []themes.Theme
	themeSel  int

	status string
}

func newOverlay(items []paletteItem) *overlay {
	o := &overlay{items: items}
	o.filter()
	return o
}

func (o *overlay) filter() {
	ranked := fuzzy.Rank(o.query, o.items, func(it paletteItem) string {
		return it.key
	})
	o.view = o.view[:0]
	for _, r := range ranked {
		o.view = append(o.view, r.Item)
	}
	if o.sel >= len(o.view) {
		o.sel = 0
	}
}

// moveSel advances a list cursor with wrap-around; every pane shares it.
func (o *overlay) moveSel(sel *int, delta, n int) {
	if n == 0 {
		return
	}
	*sel = (*sel + delta + n) % n
}

// enterDetail switches to the block-detail pane, resetting the wrap cache.
func (o *overlay) enterDetail(b *blocks.Block) {
	o.detail = b
	o.detailTop = 0
	o.detailLines = nil
	o.detailWidth = 0
	o.mode = modeBlockDetail
}

// scrollStart returns the first visible index keeping sel on screen.
func scrollStart(sel, visible int) int {
	if sel >= visible {
		return sel - visible + 1
	}
	return 0
}

// render paints the current mode into a frame.
func (o *overlay) render(cols, rows int) string {
	f := newFrame(cols, rows)
	switch o.mode {
	case modePalette:
		o.renderPalette(f)
	case modeBlocks:
		o.renderBlocks(f)
	case modeBlockDetail:
		o.renderDetail(f)
	case modeAI:
		o.renderAI(f)
	case modeThemes:
		o.renderThemes(f)
	case modeWorkflowArgs:
		o.renderArgs(f)
	}
	if o.status != "" {
		f.line(f.rows-1, sgrYellow+o.status)
	}
	return f.String()
}

func (o *overlay) renderPalette(f *frame) {
	f.line(0, sgrBold+" yarp "+sgrReset+sgrDim+" — type to search history, workflows, actions"+sgrReset)
	f.line(1, sgrCyan+" ▸ "+sgrReset+o.query+sgrInverse+" "+sgrReset)
	visible := f.rows - 4
	start := scrollStart(o.sel, visible)
	for i := 0; i < visible && start+i < len(o.view); i++ {
		it := o.view[start+i]
		marker, style := "   ", ""
		if start+i == o.sel {
			marker, style = " ● ", sgrInverse
		}
		tag := map[itemKind]string{itemCommand: "cmd", itemWorkflow: " wf", itemAction: "act"}[it.kind]
		row := fmt.Sprintf("%s%s%s %s", marker, sgrDim+tag+sgrReset+style, style, it.label)
		if it.detail != "" {
			row += sgrDim + "  " + it.detail
		}
		f.line(2+i, style+row)
	}
	f.line(f.rows-1, sgrDim+" enter insert · esc close · ↑↓ move"+sgrReset)
}

func (o *overlay) renderBlocks(f *frame) {
	f.line(0, sgrBold+" blocks "+sgrReset+sgrDim+fmt.Sprintf(" — %d recent commands", len(o.blockList))+sgrReset)
	visible := f.rows - 3
	start := scrollStart(o.blockSel, visible)
	for i := 0; i < visible && start+i < len(o.blockList); i++ {
		b := o.blockList[start+i]
		style := ""
		if start+i == o.blockSel {
			style = sgrInverse
		}
		mark := sgrGreen + "✓" + sgrReset + style
		if b.ExitCode > 0 {
			mark = sgrRed + "✗" + sgrReset + style
		}
		dur := ""
		if d := b.Duration(); d > 0 {
			dur = " · " + d.Round(10*time.Millisecond).String()
		}
		f.line(1+i, fmt.Sprintf("%s %s %s%s%s", style, mark, firstLine(b.Cmd), sgrDim+dur+" · "+shorten(b.CWD, 30), sgrReset))
	}
	f.line(f.rows-1, sgrDim+" enter view · i insert · esc back"+sgrReset)
}

func (o *overlay) renderDetail(f *frame) {
	b := o.detail
	f.line(0, sgrBold+" $ "+firstLine(b.Cmd)+sgrReset)
	meta := fmt.Sprintf(" exit %d · %s · %s", b.ExitCode, b.Duration().Round(time.Millisecond), b.CWD)
	f.line(1, sgrDim+meta+sgrReset)
	if o.detailLines == nil || o.detailWidth != f.cols-2 {
		o.detailLines = wrapText(b.Output, f.cols-2)
		o.detailWidth = f.cols - 2
	}
	lines := o.detailLines
	visible := f.rows - 4
	if o.detailTop > len(lines)-visible {
		o.detailTop = len(lines) - visible
	}
	if o.detailTop < 0 {
		o.detailTop = 0
	}
	for i := 0; i < visible && o.detailTop+i < len(lines); i++ {
		f.line(3+i, " "+lines[o.detailTop+i])
	}
	extra := ""
	if b.Truncated {
		extra = " · output truncated"
	}
	f.line(f.rows-1, sgrDim+" ↑↓/pgup/pgdn scroll · i insert · c copy · esc back"+extra+sgrReset)
}

func (o *overlay) renderAI(f *frame) {
	f.line(0, sgrBold+" ask yarp "+sgrReset+sgrDim+" — local model, /remember <fact> to save memory"+sgrReset)
	// Only the tail of a long conversation can be visible; don't re-wrap
	// the whole transcript on every streamed delta.
	chat := o.chat
	if len(chat) > 30 {
		chat = chat[len(chat)-30:]
	}
	var lines []string
	for _, c := range chat {
		prefix := map[string]string{"you": sgrCyan + "you" + sgrReset, "yarp": sgrGreen + "yarp" + sgrReset, "note": sgrDim + "  ·" + sgrReset}[c.role]
		for j, l := range wrapText(c.text, f.cols-8) {
			if j == 0 {
				lines = append(lines, fmt.Sprintf("%s  %s", prefix, l))
			} else {
				lines = append(lines, "     "+l)
			}
		}
		lines = append(lines, "")
	}
	visible := f.rows - 4
	start := 0
	if len(lines) > visible {
		start = len(lines) - visible
	}
	for i := 0; start+i < len(lines) && i < visible; i++ {
		f.line(1+i, lines[start+i])
	}
	cursor := sgrInverse + " " + sgrReset
	if o.streaming {
		cursor = sgrDim + " …thinking (esc cancels)" + sgrReset
	}
	f.line(f.rows-2, sgrCyan+" ? "+sgrReset+o.aiInput+cursor)
	hint := " enter send · esc back"
	for i, s := range o.suggested {
		hint += fmt.Sprintf(" · %d insert `%s`", i+1, shorten(s, 24))
	}
	f.line(f.rows-1, sgrDim+hint+sgrReset)
}

func (o *overlay) renderThemes(f *frame) {
	f.line(0, sgrBold+" themes "+sgrReset+sgrDim+" — applied to this terminal via OSC; drop Warp YAML themes in ~/.yarp/themes"+sgrReset)
	visible := f.rows - 3
	start := scrollStart(o.themeSel, visible)
	for i := 0; i < visible && start+i < len(o.themeList); i++ {
		t := o.themeList[start+i]
		style := ""
		if start+i == o.themeSel {
			style = sgrInverse
		}
		f.line(1+i, fmt.Sprintf("%s  %s %s", style, t.Name, sgrDim+t.Background+" / "+t.Foreground))
	}
	f.line(f.rows-1, sgrDim+" enter apply · esc back"+sgrReset)
}

func (o *overlay) renderArgs(f *frame) {
	w := o.wf
	f.line(0, sgrBold+" workflow: "+w.Name+sgrReset)
	f.line(1, sgrDim+" "+w.Command+sgrReset)
	arg := w.Arguments[o.argIdx]
	label := arg.Name
	if arg.Description != "" {
		label += " — " + arg.Description
	}
	f.line(3, fmt.Sprintf(" %s (%d/%d)", label, o.argIdx+1, len(w.Arguments)))
	def := ""
	if arg.DefaultValue != "" {
		def = sgrDim + "  [default: " + arg.DefaultValue + "]" + sgrReset
	}
	f.line(4, sgrCyan+" ▸ "+sgrReset+o.argInput+sgrInverse+" "+sgrReset+def)
	f.line(f.rows-1, sgrDim+" enter next · esc cancel"+sgrReset)
}

func firstLine(s string) string {
	if i := strings.IndexByte(s, '\n'); i >= 0 {
		return s[:i] + " …"
	}
	return s
}

func shorten(s string, n int) string {
	r := []rune(s)
	if len(r) <= n {
		return s
	}
	return "…" + string(r[len(r)-n+1:])
}
