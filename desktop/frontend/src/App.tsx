import { KeyboardEvent as ReactKeyboardEvent, PointerEvent as ReactPointerEvent, useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { BlockUpdate, SessionInfo } from './backend';
import { BlockInspector } from './BlockInspector';
import { hydrateBlocks, subscribeToBlocks } from './blockEvents';
import { TerminalPane } from './TerminalPane';
import { destroyTerminals } from './terminalRegistry';
import {
  findPane,
  mapPane,
  newTab,
  paneIDs,
  PaneNode,
  removePane,
  splitPane,
  SplitDirection,
  updateSplitRatio,
  WorkspaceTab,
} from './workspace';

type CloseTarget = { tabID: string; paneID?: string };

function PaneTree({
  node,
  active,
  focusedPaneID,
  onFocus,
  onReady,
  onSplit,
  onClose,
  onRatio,
  onStatus,
}: {
  node: PaneNode;
  active: boolean;
  focusedPaneID: string;
  onFocus(paneID: string): void;
  onReady(paneID: string, info: SessionInfo): void;
  onSplit(paneID: string, direction: SplitDirection): void;
  onClose(paneID: string): void;
  onRatio(splitID: string, ratio: number): void;
  onStatus(message: string, error?: boolean): void;
}) {
  if (node.kind === 'terminal') {
    return (
      <TerminalPane
        pane={node}
        active={active}
        focused={node.id === focusedPaneID}
        onFocus={() => onFocus(node.id)}
        onReady={(info) => onReady(node.id, info)}
        onSplit={(direction) => onSplit(node.id, direction)}
        onClose={() => onClose(node.id)}
        onStatus={onStatus}
      />
    );
  }

  const isRight = node.direction === 'right';
  const style = isRight
    ? { gridTemplateColumns: `minmax(0, ${node.ratio}fr) 6px minmax(0, ${1 - node.ratio}fr)` }
    : { gridTemplateRows: `minmax(0, ${node.ratio}fr) 6px minmax(0, ${1 - node.ratio}fr)` };

  const startResize = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    const split = event.currentTarget.parentElement;
    if (!split) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    const update = (next: PointerEvent) => {
      const rect = split.getBoundingClientRect();
      const ratio = isRight
        ? (next.clientX - rect.left) / rect.width
        : (next.clientY - rect.top) / rect.height;
      onRatio(node.id, ratio);
    };
    const stop = () => {
      window.removeEventListener('pointermove', update);
      window.removeEventListener('pointerup', stop);
    };
    window.addEventListener('pointermove', update);
    window.addEventListener('pointerup', stop, { once: true });
  };

  const shared = { active, focusedPaneID, onFocus, onReady, onSplit, onClose, onRatio, onStatus };
  return (
    <div className={`pane-split ${isRight ? 'split-right' : 'split-down'}`} style={style}>
      <PaneTree node={node.first} {...shared} />
      <div
        className="split-handle"
        role="separator"
        tabIndex={0}
        aria-orientation={isRight ? 'vertical' : 'horizontal'}
        aria-valuemin={15}
        aria-valuemax={85}
        aria-valuenow={Math.round(node.ratio * 100)}
        aria-label={`Resize ${isRight ? 'left and right' : 'upper and lower'} panes`}
        onPointerDown={startResize}
        onKeyDown={(event: ReactKeyboardEvent<HTMLDivElement>) => {
          const decrease = isRight ? event.key === 'ArrowLeft' : event.key === 'ArrowUp';
          const increase = isRight ? event.key === 'ArrowRight' : event.key === 'ArrowDown';
          if (!decrease && !increase) return;
          event.preventDefault();
          const step = event.shiftKey ? 0.1 : 0.02;
          onRatio(node.id, node.ratio + (increase ? step : -step));
        }}
      />
      <PaneTree node={node.second} {...shared} />
    </div>
  );
}

export function App() {
  const [tabs, setTabs] = useState<WorkspaceTab[]>(() => [newTab()]);
  const [activeTabID, setActiveTabID] = useState(() => tabs[0].id);
  const [closeTarget, setCloseTarget] = useState<CloseTarget | null>(null);
  const [closing, setClosing] = useState(false);
  const [blocksOpen, setBlocksOpen] = useState(false);
  const [blockUpdates, setBlockUpdates] = useState<BlockUpdate[]>([]);
  const [status, setStatus] = useState({ message: 'Starting local session', error: false });
  const keepOpenRef = useRef<HTMLButtonElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const closingRef = useRef(false);
  closingRef.current = closing;

  const activeTab = useMemo(() => tabs.find((tab) => tab.id === activeTabID) ?? tabs[0], [tabs, activeTabID]);

  useEffect(() => {
    const unsubscribe = subscribeToBlocks(setBlockUpdates);
    void hydrateBlocks().catch((error: unknown) => {
      setStatus({ message: error instanceof Error ? error.message : String(error), error: true });
    });
    return unsubscribe;
  }, []);

  const updateTab = useCallback((tabID: string, update: (tab: WorkspaceTab) => WorkspaceTab) => {
    setTabs((current) => current.map((tab) => tab.id === tabID ? update(tab) : tab));
  }, []);

  const addTab = useCallback((cwd = '') => {
    const tab = newTab(cwd);
    setTabs((current) => [...current, tab]);
    setActiveTabID(tab.id);
  }, []);

  const requestClose = useCallback((tabID: string, paneID?: string) => {
    setCloseTarget({ tabID, paneID });
  }, []);

  const commitClose = useCallback(async () => {
    if (!closeTarget) return;
    const { tabID, paneID } = closeTarget;
    const closingTab = tabs.find((item) => item.id === tabID);
    if (!closingTab) {
      setCloseTarget(null);
      return;
    }
    const closingPaneIDs = paneID && paneIDs(closingTab.root).length > 1
      ? [paneID]
      : paneIDs(closingTab.root);
    setClosing(true);
    try {
      await destroyTerminals(closingPaneIDs);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setStatus({ message: `Could not close session: ${message}`, error: true });
      setClosing(false);
      return;
    }

    let nextTabs: WorkspaceTab[];
    if (paneID && paneIDs(closingTab.root).length > 1) {
      const root = removePane(closingTab.root, paneID);
      if (!root) {
        setClosing(false);
        return;
      }
      const remaining = paneIDs(root);
      nextTabs = tabs.map((item) => item.id === tabID
        ? { ...item, root, focusedPaneID: remaining.includes(item.focusedPaneID) ? item.focusedPaneID : remaining[0] }
        : item);
    } else {
      nextTabs = tabs.filter((item) => item.id !== tabID);
      if (nextTabs.length === 0) {
        nextTabs = [newTab(closingTab.root.kind === 'terminal' ? closingTab.root.cwd : '')];
      }
    }
    setTabs(nextTabs);
    if (activeTabID === tabID && !nextTabs.some((tab) => tab.id === activeTabID)) {
      const oldIndex = tabs.indexOf(closingTab);
      setActiveTabID(nextTabs[Math.min(Math.max(0, oldIndex - 1), nextTabs.length - 1)].id);
    }
    setClosing(false);
    setCloseTarget(null);
  }, [activeTabID, closeTarget, tabs]);

  const splitFocused = useCallback((direction: SplitDirection) => {
    if (!activeTab) return;
    updateTab(activeTab.id, (tab) => {
      const result = splitPane(tab.root, tab.focusedPaneID, direction);
      return { ...tab, root: result.root, focusedPaneID: result.newPaneID };
    });
  }, [activeTab, updateTab]);

  const focusSpatialPane = useCallback((direction: 'left' | 'right' | 'up' | 'down') => {
    if (!activeTab) return;
    const elements = Array.from(document.querySelectorAll<HTMLElement>('.tab-workspace.active [data-pane-id]'));
    const current = elements.find((element) => element.dataset.paneId === activeTab.focusedPaneID);
    if (!current) return;
    const source = current.getBoundingClientRect();
    const sourceX = source.left + source.width / 2;
    const sourceY = source.top + source.height / 2;
    const candidates = elements.flatMap((element) => {
      if (element === current || !element.dataset.paneId) return [];
      const rect = element.getBoundingClientRect();
      const dx = rect.left + rect.width / 2 - sourceX;
      const dy = rect.top + rect.height / 2 - sourceY;
      const eligible = direction === 'left' ? dx < -1
        : direction === 'right' ? dx > 1
          : direction === 'up' ? dy < -1
            : dy > 1;
      if (!eligible) return [];
      const primary = direction === 'left' || direction === 'right' ? Math.abs(dx) : Math.abs(dy);
      const cross = direction === 'left' || direction === 'right' ? Math.abs(dy) : Math.abs(dx);
      return [{ paneID: element.dataset.paneId, score: primary + cross * 0.45 }];
    });
    candidates.sort((a, b) => a.score - b.score);
    if (candidates[0]) updateTab(activeTab.id, (tab) => ({ ...tab, focusedPaneID: candidates[0].paneID }));
  }, [activeTab, updateTab]);

  useEffect(() => {
    if (!closeTarget) return;
    previousFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    requestAnimationFrame(() => keepOpenRef.current?.focus());
    const modalKeys = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !closingRef.current) {
        event.preventDefault();
        setCloseTarget(null);
        return;
      }
      if (event.key !== 'Tab') return;
      const first = keepOpenRef.current;
      const last = closeButtonRef.current;
      if (!first || !last) return;
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener('keydown', modalKeys);
    return () => {
      window.removeEventListener('keydown', modalKeys);
      previousFocusRef.current?.focus();
    };
  }, [closeTarget]);

  useEffect(() => {
    const shortcuts = (event: KeyboardEvent) => {
      if (!event.metaKey) return;
      if (event.shiftKey && event.key.toLowerCase() === 'b') {
        event.preventDefault();
        setBlocksOpen((open) => !open);
      } else if (event.key.toLowerCase() === 't') {
        event.preventDefault();
        addTab(activeTab && findPane(activeTab.root, activeTab.focusedPaneID)?.cwd);
      } else if (event.key.toLowerCase() === 'w') {
        event.preventDefault();
        if (activeTab) requestClose(activeTab.id, activeTab.focusedPaneID);
      } else if (event.key.toLowerCase() === 'd') {
        event.preventDefault();
        splitFocused(event.shiftKey ? 'down' : 'right');
      } else if (event.altKey && (event.key === 'ArrowRight' || event.key === 'ArrowDown')) {
        event.preventDefault();
        focusSpatialPane(event.key === 'ArrowRight' ? 'right' : 'down');
      } else if (event.altKey && (event.key === 'ArrowLeft' || event.key === 'ArrowUp')) {
        event.preventDefault();
        focusSpatialPane(event.key === 'ArrowLeft' ? 'left' : 'up');
      } else if (/^[1-9]$/.test(event.key)) {
        const tab = tabs[Number(event.key) - 1];
        if (tab) {
          event.preventDefault();
          setActiveTabID(tab.id);
        }
      }
    };
    window.addEventListener('keydown', shortcuts);
    return () => window.removeEventListener('keydown', shortcuts);
  }, [activeTab, addTab, focusSpatialPane, requestClose, splitFocused, tabs]);

  return (
    <div className="app-shell">
      <header className="titlebar">
        <div className="wordmark" aria-label="Yarp"><span className="wordmark-mark">Y</span><span>YARP</span></div>
        <div className="titlebar-context"><span className="context-label">WORKSPACE</span><span className="context-name">Sandford</span></div>
        <div className="local-badge"><span /> LOCAL</div>
      </header>

      <div className="app-body">
        <aside className="sidebar">
          <div className="sidebar-section-label">THIS MACHINE</div>
          <button className={`nav-item ${!blocksOpen ? 'active' : ''}`} type="button" onClick={() => setBlocksOpen(false)}>
            <span className="nav-icon">›_</span><span>Terminal</span><kbd>{tabs.length}</kbd>
          </button>
          <button className={`nav-item ${blocksOpen ? 'active' : ''}`} type="button" onClick={() => setBlocksOpen(true)}>
            <span className="nav-icon">▤</span><span>Blocks</span><kbd>{blockUpdates.length}</kbd>
          </button>
          <div className="sidebar-spacer" />
          <div className="shortcut-card">
            <span className="principle-kicker">DESK CONTROLS</span>
            <span><kbd>⌘T</kbd> new beat</span>
            <span><kbd>⌘D</kbd> split right</span>
            <span><kbd>⌘⇧D</kbd> split down</span>
            <span><kbd>⌘⌥←</kbd> focus pane</span>
          </div>
          <div className="principle-card">
            <span className="principle-kicker">THE GREATER GOOD</span>
            <strong>Your shell.<br />Your machine.<br />Your history.</strong>
            <span className="principle-note">No account. No cloud.</span>
          </div>
        </aside>

        <main className="workspace">
          <div className="tabstrip" role="tablist" aria-label="Terminal beats">
            {tabs.map((tab, index) => (
              <div
                className={`tab ${tab.id === activeTabID ? 'active' : ''}`}
                role="presentation"
                key={tab.id}
              >
                <button
                  className="tab-main"
                  type="button"
                  role="tab"
                  aria-selected={tab.id === activeTabID}
                  onClick={() => setActiveTabID(tab.id)}
                >
                  <span className="tab-status" />
                  <span className="tab-copy"><span className="tab-title">{tab.title}</span><span className="tab-path">{findPane(tab.root, tab.focusedPaneID)?.cwd || 'local'}</span></span>
                  <kbd>{index + 1}</kbd>
                </button>
                <button className="tab-close" type="button" aria-label={`Close ${tab.title}`} onClick={() => requestClose(tab.id)}>×</button>
              </div>
            ))}
            <button className="new-tab" type="button" title="New beat (⌘T)" aria-label="New terminal beat" onClick={() => addTab(activeTab && findPane(activeTab.root, activeTab.focusedPaneID)?.cwd)}>+</button>
            <div className="tabstrip-fill" />
            <div className="session-chip"><span />{paneIDs(activeTab.root).length} {paneIDs(activeTab.root).length === 1 ? 'PANE' : 'PANES'}</div>
          </div>

          <div className={`workspace-canvas ${blocksOpen ? 'inspector-open' : ''}`}>
            {tabs.map((tab) => (
              <div className={`tab-workspace ${tab.id === activeTabID ? 'active' : ''}`} key={tab.id}>
                <PaneTree
                  node={tab.root}
                  active={tab.id === activeTabID}
                  focusedPaneID={tab.focusedPaneID}
                  onFocus={(paneID) => updateTab(tab.id, (item) => ({ ...item, focusedPaneID: paneID }))}
                  onReady={(paneID, info) => updateTab(tab.id, (item) => ({
                    ...item,
                    title: item.title === 'Terminal' ? info.shell : item.title,
                    root: mapPane(item.root, paneID, (pane) => ({ ...pane, title: info.shell, cwd: info.cwd })),
                  }))}
                  onSplit={(paneID, direction) => updateTab(tab.id, (item) => {
                    const result = splitPane(item.root, paneID, direction);
                    return { ...item, root: result.root, focusedPaneID: result.newPaneID };
                  })}
                  onClose={(paneID) => requestClose(tab.id, paneID)}
                  onRatio={(splitID, ratio) => updateTab(tab.id, (item) => ({ ...item, root: updateSplitRatio(item.root, splitID, ratio) }))}
                  onStatus={(message, error = false) => setStatus({ message, error })}
                />
              </div>
            ))}
            {blocksOpen && (
              <BlockInspector
                blocks={blockUpdates}
                onClose={() => setBlocksOpen(false)}
                onStatus={(message, error = false) => setStatus({ message, error })}
              />
            )}
          </div>

          <footer className={`statusbar ${status.error ? 'error' : ''}`}>
            <span><span className="status-led" />{status.message}</span>
            <span className="status-promise">standalone desktop · {tabs.length} {tabs.length === 1 ? 'beat' : 'beats'} · real PTYs</span>
          </footer>
        </main>
      </div>

      {closeTarget && (
        <div className="modal-backdrop" role="presentation" onMouseDown={() => { if (!closing) setCloseTarget(null); }}>
          <section className="close-dialog" role="alertdialog" aria-modal="true" aria-labelledby="close-title" onMouseDown={(event) => event.stopPropagation()}>
            <span className="dialog-kicker">END LOCAL SESSION</span>
            <h2 id="close-title">Close this {closeTarget.paneID ? 'pane' : 'beat'}?</h2>
            <p>Yarp will ask each local shell in this {closeTarget.paneID ? 'pane' : 'beat'} to stop before removing it from the desk.</p>
            <div className="dialog-actions">
              <button ref={keepOpenRef} type="button" disabled={closing} onClick={() => setCloseTarget(null)}>Keep open</button>
              <button ref={closeButtonRef} className="danger" type="button" disabled={closing} onClick={() => { void commitClose(); }}>{closing ? 'Stopping…' : 'Close session'}</button>
            </div>
          </section>
        </div>
      )}
    </div>
  );
}
