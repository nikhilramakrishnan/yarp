import { useEffect, useRef, useState } from 'react';
import '@xterm/xterm/css/xterm.css';

import { SessionInfo } from './backend';
import { attachTerminal, fitTerminal, focusTerminal, TerminalSnapshot } from './terminalRegistry';
import { SplitDirection, TerminalLeaf } from './workspace';

type Props = {
  pane: TerminalLeaf;
  active: boolean;
  focused: boolean;
  onFocus(): void;
  onReady(info: SessionInfo): void;
  onSplit(direction: SplitDirection): void;
  onClose(): void;
  onStatus(message: string, error?: boolean): void;
};

const initialSnapshot: TerminalSnapshot = { shell: 'local shell', size: '—', state: 'starting' };

export function TerminalPane({ pane, active, focused, onFocus, onReady, onSplit, onClose, onStatus }: Props) {
  const hostRef = useRef<HTMLDivElement>(null);
  const callbacks = useRef({ onReady, onStatus });
  const [snapshot, setSnapshot] = useState<TerminalSnapshot>(initialSnapshot);
  callbacks.current = { onReady, onStatus };

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    return attachTerminal(pane.id, host, pane.cwd, {
      onReady: (info) => callbacks.current.onReady(info),
      onSnapshot: setSnapshot,
      onStatus: (message, error) => callbacks.current.onStatus(message, error),
    });
  }, [pane.id, pane.cwd]);

  useEffect(() => {
    if (!active) return;
    fitTerminal(pane.id);
    if (focused) focusTerminal(pane.id);
  }, [active, focused, pane.id]);

  return (
    <section
      className={`terminal-pane ${focused ? 'focused' : ''}`}
      data-pane-id={pane.id}
      aria-label={`${snapshot.shell} terminal pane`}
      onMouseDown={onFocus}
    >
      <div className="terminal-header">
        <div className="terminal-identity">
          <span className="prompt-glyph">›_</span>
          <span>{snapshot.shell}</span>
          <span className={`pane-state ${snapshot.state}`}>{snapshot.state}</span>
        </div>
        <div className="pane-actions">
          <span className="terminal-size">{snapshot.size}</span>
          <button type="button" title="Split right (⌘D)" aria-label="Split terminal right" onClick={() => onSplit('right')}>◫</button>
          <button type="button" title="Split down (⌘⇧D)" aria-label="Split terminal down" onClick={() => onSplit('down')}>⊟</button>
          <button type="button" title="Close pane (⌘W)" aria-label="Close terminal pane" onClick={onClose}>×</button>
        </div>
      </div>
      <div className="terminal-wrap">
        <div className="terminal-host" ref={hostRef} />
      </div>
    </section>
  );
}
