// Command yarp is the local-first terminal client: the entire product in one
// static, cross-platform binary.
//
//	yarp                 start a shell session with blocks + overlay (ctrl-g)
//	yarp history [q]     search command history
//	yarp ai [question]   ask the local model, with recent-session context
//	yarp memory ...      inspect the agent's memory
//	yarp backup ...      snapshot ~/.yarp to places you own
//	yarp themes ...      list or set themes
//	yarp workflows       list workflows
//	yarp init <shell>    print the shell-integration snippet
//	yarp doctor          check the environment
package main

import (
	"bufio"
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/nikhilramakrishnan/yarp/go/internal/agent"
	"github.com/nikhilramakrishnan/yarp/go/internal/backup"
	"github.com/nikhilramakrishnan/yarp/go/internal/blocks"
	"github.com/nikhilramakrishnan/yarp/go/internal/config"
	"github.com/nikhilramakrishnan/yarp/go/internal/fuzzy"
	"github.com/nikhilramakrishnan/yarp/go/internal/llm"
	"github.com/nikhilramakrishnan/yarp/go/internal/shellhook"
	"github.com/nikhilramakrishnan/yarp/go/internal/themes"
	"github.com/nikhilramakrishnan/yarp/go/internal/ui"
	"github.com/nikhilramakrishnan/yarp/go/internal/workflows"
)

var version = "dev" // set via -ldflags "-X main.version=..."

func main() {
	settings, err := config.Load()
	if err != nil {
		fatal(err)
	}
	args := os.Args[1:]
	if len(args) == 0 {
		code, err := ui.Run(settings)
		if err != nil {
			fatal(err)
		}
		os.Exit(code)
	}
	switch args[0] {
	case "history":
		err = cmdHistory(args[1:])
	case "ai":
		err = cmdAI(settings, args[1:])
	case "memory":
		err = cmdMemory(args[1:])
	case "backup":
		err = cmdBackup(settings, args[1:])
	case "themes":
		err = cmdThemes(settings, args[1:])
	case "workflows":
		err = cmdWorkflows()
	case "init":
		err = cmdInit(args[1:])
	case "doctor":
		err = cmdDoctor(settings)
	case "version", "--version", "-v":
		fmt.Println("yarp", version)
	case "help", "--help", "-h":
		fmt.Print(usage)
	default:
		fmt.Fprintf(os.Stderr, "yarp: unknown command %q\n\n%s", args[0], usage)
		os.Exit(2)
	}
	if err != nil {
		fatal(err)
	}
}

const usage = `yarp — a fast, local-first terminal client

usage:
  yarp                     start a session (overlay on ctrl-g)
  yarp history [query]     search command history
  yarp ai <question>       ask the local model (needs Ollama/LM Studio/llama.cpp)
  yarp memory list|add <fact>|clear
  yarp backup              snapshot ~/.yarp and ship to configured targets
  yarp backup restore <archive> [dest]
  yarp backup gdrive-auth  authorize your own Google Drive OAuth client
  yarp themes [set <name>]
  yarp workflows           list workflows from ~/.yarp/workflows
  yarp init <shell>        print shell integration (bash|zsh|fish|pwsh)
  yarp doctor              check shells, model runtimes, and data dir
  yarp version
`

func fatal(err error) {
	fmt.Fprintln(os.Stderr, "yarp:", err)
	os.Exit(1)
}

func historyDir() (string, error) { return config.Subdir("history") }

func cmdHistory(args []string) error {
	dir, err := historyDir()
	if err != nil {
		return err
	}
	recent, err := blocks.LoadRecent(dir, 500)
	if err != nil {
		return err
	}
	query := strings.Join(args, " ")
	if query != "" {
		ranked := fuzzy.Rank(query, recent, func(b blocks.Block) string { return b.Cmd })
		recent = recent[:0]
		for _, r := range ranked {
			recent = append(recent, r.Item)
		}
	}
	if len(recent) > 50 {
		recent = recent[:50]
	}
	for i := len(recent) - 1; i >= 0; i-- {
		b := recent[i]
		status := "✓"
		switch {
		case b.ExitCode > 0:
			status = "✗"
		case b.ExitCode < 0:
			status = "?"
		}
		fmt.Printf("%s %s  %s\n", b.StartedAt.Format("01-02 15:04"), status, b.Cmd)
	}
	return nil
}

func cmdAI(settings *config.Settings, args []string) error {
	question := strings.Join(args, " ")
	if question == "" {
		return fmt.Errorf("usage: yarp ai <question>")
	}
	ctx := context.Background()
	client, err := llm.Discover(ctx, settings.LLM.Endpoint, settings.LLM.Model)
	if err != nil {
		return err
	}
	dir, err := historyDir()
	if err != nil {
		return err
	}
	recent, _ := blocks.LoadRecent(dir, settings.LLM.ContextBlocks)
	ag := &agent.Agent{Client: client}
	if settings.MemoryOn() {
		base, _ := config.Dir()
		ag.Memory = llm.OpenMemory(filepath.Join(base, "memory.jsonl"))
	}
	cwd, _ := os.Getwd()
	_, err = ag.Reply(ctx, question, recent, cwd, func(d string) { fmt.Print(d) })
	fmt.Println()
	return err
}

func cmdMemory(args []string) error {
	base, err := config.Dir()
	if err != nil {
		return err
	}
	mem := llm.OpenMemory(filepath.Join(base, "memory.jsonl"))
	sub := "list"
	if len(args) > 0 {
		sub = args[0]
	}
	switch sub {
	case "list":
		entries, err := mem.All()
		if err != nil {
			return err
		}
		if len(entries) == 0 {
			fmt.Println("no memories yet — use `yarp memory add <fact>` or /remember in the AI pane")
			return nil
		}
		for _, e := range entries {
			fmt.Printf("%s  %s\n", e.At.Format("2006-01-02"), e.Text)
		}
	case "add":
		fact := strings.Join(args[1:], " ")
		if fact == "" {
			return fmt.Errorf("usage: yarp memory add <fact>")
		}
		return mem.Remember(fact, time.Now())
	case "clear":
		return os.Remove(filepath.Join(base, "memory.jsonl"))
	default:
		return fmt.Errorf("usage: yarp memory list|add <fact>|clear")
	}
	return nil
}

func cmdBackup(settings *config.Settings, args []string) error {
	home, err := config.Dir()
	if err != nil {
		return err
	}
	if len(args) > 0 {
		switch args[0] {
		case "restore":
			if len(args) < 2 {
				return fmt.Errorf("usage: yarp backup restore <archive> [dest]")
			}
			dest := home
			if len(args) > 2 {
				dest = args[2]
			}
			if err := backup.Restore(args[1], dest); err != nil {
				return err
			}
			fmt.Println("restored to", dest)
			return nil
		case "gdrive-auth":
			return gdriveAuth(settings)
		}
	}
	summary, err := backup.Run(home, backup.RunConfig{
		Dir:          settings.Backup.Dir,
		RcloneRemote: settings.Backup.RcloneRemote,
		Keep:         settings.Backup.Keep,
		Drive: backup.DriveAuth{
			ClientID:     settings.Backup.GoogleDrive.ClientID,
			ClientSecret: settings.Backup.GoogleDrive.ClientSecret,
			RefreshToken: settings.Backup.GoogleDrive.RefreshToken,
		},
		DriveFolder: settings.Backup.GoogleDrive.FolderID,
	}, time.Now())
	if err != nil {
		return err
	}
	fmt.Println(summary)
	return nil
}

func gdriveAuth(settings *config.Settings) error {
	gd := &settings.Backup.GoogleDrive
	in := bufio.NewReader(os.Stdin)
	if gd.ClientID == "" || gd.ClientSecret == "" {
		fmt.Println("yarp ships no cloud credentials: create an OAuth client (type: TV and")
		fmt.Println("Limited Input) in your own Google Cloud project with the Drive API on,")
		fmt.Println("then paste its ID and secret. They are stored only in ~/.yarp/settings.json.")
		fmt.Print("client id: ")
		id, _ := in.ReadString('\n')
		fmt.Print("client secret: ")
		secret, _ := in.ReadString('\n')
		gd.ClientID = strings.TrimSpace(id)
		gd.ClientSecret = strings.TrimSpace(secret)
		if gd.ClientID == "" || gd.ClientSecret == "" {
			return fmt.Errorf("both client id and secret are required")
		}
	}
	ctx := context.Background()
	prompt, err := backup.StartDeviceAuth(ctx, gd.ClientID)
	if err != nil {
		return err
	}
	fmt.Printf("\nOn any device, open  %s  and enter code  %s\nWaiting for approval…\n",
		prompt.VerificationURL, prompt.UserCode)
	token, err := prompt.Wait(ctx, gd.ClientID, gd.ClientSecret)
	if err != nil {
		return err
	}
	gd.RefreshToken = token
	if err := settings.Save(); err != nil {
		return err
	}
	fmt.Println("Google Drive connected. `yarp backup` now uploads snapshots to your Drive.")
	return nil
}

func cmdThemes(settings *config.Settings, args []string) error {
	base, err := config.Dir()
	if err != nil {
		return err
	}
	dir := filepath.Join(base, "themes")
	if len(args) >= 2 && args[0] == "set" {
		name := strings.Join(args[1:], " ")
		t, ok := themes.Find(dir, name)
		if !ok {
			return fmt.Errorf("no theme named %q (see `yarp themes`)", name)
		}
		os.Stdout.WriteString(t.OSC())
		settings.Theme = t.Name
		return settings.Save()
	}
	for _, t := range themes.Load(dir) {
		active := " "
		if strings.EqualFold(t.Name, settings.Theme) {
			active = "*"
		}
		fmt.Printf("%s %-14s %s on %s\n", active, t.Name, t.Foreground, t.Background)
	}
	return nil
}

func cmdWorkflows() error {
	dir, err := config.Subdir("workflows")
	if err != nil {
		return err
	}
	wfs, err := workflows.Load(dir)
	if err != nil {
		return err
	}
	if len(wfs) == 0 {
		fmt.Printf("no workflows in %s — drop Warp-format YAML files there\n", dir)
		return nil
	}
	for _, w := range wfs {
		fmt.Printf("%-30s %s\n", w.Name, w.Command)
	}
	return nil
}

func cmdInit(args []string) error {
	if len(args) != 1 {
		fmt.Print(shellhook.InitUsage() + "\n")
		os.Exit(2)
	}
	snippet, ok := shellhook.Snippet(shellhook.Kind(args[0]))
	if !ok {
		return fmt.Errorf("unsupported shell %q", args[0])
	}
	fmt.Print(snippet)
	return nil
}

func cmdDoctor(settings *config.Settings) error {
	home, err := config.Dir()
	fmt.Printf("data dir      %s (err=%v)\n", home, err)
	fmt.Printf("version       %s\n", version)

	shell := settings.Shell
	if shell == "" {
		shell = os.Getenv("SHELL")
	}
	kind := shellhook.Detect(shell)
	hooked := "yes"
	if kind == shellhook.Unknown {
		hooked = "no (blocks disabled; bash, zsh, fish, pwsh supported)"
	}
	fmt.Printf("shell         %s — integration: %s\n", shell, hooked)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	client, err := llm.Discover(ctx, settings.LLM.Endpoint, settings.LLM.Model)
	if err != nil {
		fmt.Printf("local model   none (%v)\n", err)
	} else {
		fmt.Printf("local model   %s @ %s\n", client.Model, client.BaseURL)
	}

	targets := []string{}
	if settings.Backup.Dir != "" {
		targets = append(targets, settings.Backup.Dir)
	}
	if settings.Backup.RcloneRemote != "" {
		targets = append(targets, settings.Backup.RcloneRemote)
	}
	if settings.Backup.GoogleDrive.RefreshToken != "" {
		targets = append(targets, "google drive")
	}
	if len(targets) == 0 {
		targets = append(targets, "local only (~/.yarp/backups)")
	}
	fmt.Printf("backup        %s\n", strings.Join(targets, ", "))
	return nil
}
