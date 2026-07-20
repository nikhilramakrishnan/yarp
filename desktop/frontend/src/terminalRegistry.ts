import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { Terminal } from '@xterm/xterm';

import {
  ackTerminalOutput,
  resizeTerminal,
  SessionInfo,
  startTerminal,
  stopTerminal,
  writeTerminal,
} from './backend';
import { decodeBase64, forgetTerminal, subscribeToTerminal, TerminalData, TerminalExit } from './terminalEvents';
import { ContiguousProgress, ContiguousQueue } from './sequence';

export type TerminalState = 'starting' | 'live' | 'ended' | 'error';

export type TerminalSnapshot = {
  shell: string;
  size: string;
  state: TerminalState;
};

export type TerminalCallbacks = {
  onReady(info: SessionInfo): void;
  onSnapshot(snapshot: TerminalSnapshot): void;
  onStatus(message: string, error?: boolean): void;
};

const terminalTheme = {
  background: '#0b1628',
  foreground: '#f5efd8',
  cursor: '#bd4747',
  cursorAccent: '#0b1628',
  selectionBackground: '#41516d99',
  black: '#111c2e',
  red: '#bd4747',
  green: '#7eb18a',
  yellow: '#d8b963',
  blue: '#78a6c8',
  magenta: '#a887b8',
  cyan: '#78aaa7',
  white: '#f5efd8',
  brightBlack: '#68758a',
  brightRed: '#d56363',
  brightGreen: '#9bc5a5',
  brightYellow: '#e6cc86',
  brightBlue: '#98bdd6',
  brightMagenta: '#bea2c9',
  brightCyan: '#98c2bf',
  brightWhite: '#fffaf0',
};

class TerminalController {
  readonly paneID: string;
  private readonly terminal: Terminal;
  private readonly fit = new FitAddon();
  private readonly surface = document.createElement('div');
  private sessionID = '';
  private shell = 'local shell';
  private cwd = '';
  private state: TerminalState = 'starting';
  private size = '—';
  private callbacks: TerminalCallbacks | null = null;
  private observer: ResizeObserver | null = null;
  private unsubscribe = () => {};
  private resizeTimer: number | undefined;
  private readonly incomingData = new ContiguousQueue<TerminalData>();
  private readonly renderedData = new ContiguousProgress();
  private lastAckSequence = 0;
  private pendingExit: TerminalExit | null = null;
  private started = false;
  private opened = false;
  private disposed = false;

  constructor(paneID: string) {
    this.paneID = paneID;
    this.surface.className = 'terminal-surface';
    this.terminal = new Terminal({
      convertEol: false,
      cursorBlink: true,
      cursorStyle: 'bar',
      cursorWidth: 2,
      fontFamily: '"SFMono-Regular", "Cascadia Code", "Roboto Mono", Menlo, monospace',
      fontSize: 13.5,
      letterSpacing: 0.12,
      lineHeight: 1.25,
      scrollback: 10000,
      theme: terminalTheme,
    });
    this.terminal.loadAddon(this.fit);
    this.terminal.loadAddon(new WebLinksAddon());
    this.terminal.onData((data) => {
      if (this.sessionID) void writeTerminal(this.sessionID, data).catch((error: unknown) => this.fail(error));
    });
  }

  attach(host: HTMLElement, cwd: string, callbacks: TerminalCallbacks): () => void {
    if (this.disposed) throw new Error('Cannot attach a disposed terminal.');
    this.callbacks = callbacks;
    host.replaceChildren(this.surface);
    if (!this.opened) {
      this.terminal.open(this.surface);
      this.opened = true;
    }
    this.observer?.disconnect();
    this.observer = new ResizeObserver(() => this.refreshSize());
    this.observer.observe(host);
    this.publish();
    if (this.sessionID) callbacks.onReady({ id: this.sessionID, shell: this.shell, cwd: this.cwd });
    requestAnimationFrame(() => this.refreshSize());
    if (!this.started) {
      this.started = true;
      void this.start(cwd);
    }

    return () => {
      if (this.surface.parentElement === host) {
        this.observer?.disconnect();
        this.observer = null;
        this.callbacks = null;
      }
    };
  }

  focus(): void {
    this.terminal.focus();
  }

  fitNow(): void {
    requestAnimationFrame(() => this.refreshSize());
  }

  async destroy(): Promise<void> {
    if (this.disposed) return;
    const sessionID = this.sessionID;
    if (sessionID) await stopTerminal(sessionID);
    this.disposed = true;
    this.observer?.disconnect();
    this.unsubscribe();
    if (sessionID) forgetTerminal(sessionID);
    window.clearTimeout(this.resizeTimer);
    this.sessionID = '';
    this.terminal.dispose();
    this.surface.remove();
    if (sessionID) await stopTerminal(sessionID);
  }

  private async start(cwd: string): Promise<void> {
    try {
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      this.refreshSize();
      const info = await startTerminal(this.terminal.cols || 80, this.terminal.rows || 24, cwd);
      if (this.disposed) {
        await stopTerminal(info.id);
        return;
      }
      this.sessionID = info.id;
      this.shell = info.shell;
      this.cwd = info.cwd;
      this.state = 'live';
      this.callbacks?.onReady(info);
      this.callbacks?.onStatus(`Local ${info.shell} session`);
      this.publish();
      this.unsubscribe = subscribeToTerminal(info.id, {
        data: (event) => this.receiveData(info.id, event),
        exit: (exit: TerminalExit) => this.receiveExit(exit),
        cwd: (cwd) => {
          this.cwd = cwd;
          this.callbacks?.onReady({ id: info.id, shell: this.shell, cwd });
        },
      });
    } catch (error) {
      this.fail(error);
      this.terminal.writeln('\x1b[31mYarp could not start the local shell.\x1b[0m');
    }
  }

  private refreshSize(): void {
    const parent = this.surface.parentElement;
    if (this.disposed || !parent || parent.clientWidth === 0 || parent.clientHeight === 0) return;
    this.fit.fit();
    this.size = `${this.terminal.cols} × ${this.terminal.rows}`;
    this.publish();
    if (this.sessionID) {
      window.clearTimeout(this.resizeTimer);
      this.resizeTimer = window.setTimeout(() => {
        void resizeTerminal(this.sessionID, this.terminal.cols, this.terminal.rows).catch((error: unknown) => this.fail(error));
      }, 40);
    }
  }

  private handleExit({ code, message }: TerminalExit): void {
    const detail = message ? ` · ${message}` : '';
    this.terminal.writeln(`\r\n\x1b[38;2;255;204;102m[yarp: shell exited ${code}${detail}]\x1b[0m`);
    this.state = 'ended';
    const sessionID = this.sessionID;
    this.sessionID = '';
    this.unsubscribe();
    if (sessionID) forgetTerminal(sessionID);
    this.callbacks?.onStatus(`Session exited (${code})`);
    this.publish();
  }

  private receiveData(sessionID: string, event: TerminalData): void {
    if (event.sequence <= 0) {
      this.terminal.write(decodeBase64(event.encoded));
      return;
    }
    for (const next of this.incomingData.push(event.sequence, event)) {
      this.terminal.write(decodeBase64(next.value.encoded), () => this.markRendered(sessionID, next.sequence));
    }
  }

  private markRendered(sessionID: string, sequence: number): void {
    const rendered = this.renderedData.mark(sequence);
    if (rendered > this.lastAckSequence) {
      this.lastAckSequence = rendered;
      void ackTerminalOutput(sessionID, this.lastAckSequence).catch((error: unknown) => this.fail(error));
    }
    this.maybeFinishExit();
  }

  private receiveExit(exit: TerminalExit): void {
    this.pendingExit = exit;
    this.maybeFinishExit();
  }

  private maybeFinishExit(): void {
    if (!this.pendingExit || this.renderedData.value() < this.pendingExit.finalSequence) return;
    const exit = this.pendingExit;
    this.pendingExit = null;
    this.handleExit(exit);
  }

  private fail(error: unknown): void {
    const message = error instanceof Error ? error.message : String(error);
    this.state = 'error';
    this.callbacks?.onStatus(message, true);
    this.publish();
  }

  private publish(): void {
    this.callbacks?.onSnapshot({ shell: this.shell, size: this.size, state: this.state });
  }
}

const controllers = new Map<string, TerminalController>();

function controller(paneID: string): TerminalController {
  const existing = controllers.get(paneID);
  if (existing) return existing;
  const created = new TerminalController(paneID);
  controllers.set(paneID, created);
  return created;
}

export function attachTerminal(paneID: string, host: HTMLElement, cwd: string, callbacks: TerminalCallbacks): () => void {
  return controller(paneID).attach(host, cwd, callbacks);
}

export function focusTerminal(paneID: string): void {
  controller(paneID).focus();
}

export function fitTerminal(paneID: string): void {
  controller(paneID).fitNow();
}

export async function destroyTerminal(paneID: string): Promise<void> {
  const existing = controllers.get(paneID);
  if (!existing) return;
  await existing.destroy();
  if (controllers.get(paneID) === existing) controllers.delete(paneID);
}

export async function destroyTerminals(paneIDs: string[]): Promise<void> {
  await Promise.all(paneIDs.map((paneID) => destroyTerminal(paneID)));
}
