export type SplitDirection = 'right' | 'down';

export type TerminalLeaf = {
  kind: 'terminal';
  id: string;
  title: string;
  cwd: string;
};

export type SplitNode = {
  kind: 'split';
  id: string;
  direction: SplitDirection;
  ratio: number;
  first: PaneNode;
  second: PaneNode;
};

export type PaneNode = TerminalLeaf | SplitNode;

export type WorkspaceTab = {
  id: string;
  title: string;
  root: PaneNode;
  focusedPaneID: string;
};

let fallbackID = 0;

export function newID(prefix: string): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return `${prefix}-${crypto.randomUUID()}`;
  }
  fallbackID += 1;
  return `${prefix}-${Date.now()}-${fallbackID}`;
}

export function newTerminal(cwd = ''): TerminalLeaf {
  return {
    kind: 'terminal',
    id: newID('pane'),
    title: 'Terminal',
    cwd,
  };
}

export function newTab(cwd = ''): WorkspaceTab {
  const pane = newTerminal(cwd);
  return {
    id: newID('beat'),
    title: 'Terminal',
    root: pane,
    focusedPaneID: pane.id,
  };
}

export function mapPane(node: PaneNode, paneID: string, update: (pane: TerminalLeaf) => PaneNode): PaneNode {
  if (node.kind === 'terminal') {
    return node.id === paneID ? update(node) : node;
  }
  const first = mapPane(node.first, paneID, update);
  const second = mapPane(node.second, paneID, update);
  return first === node.first && second === node.second ? node : { ...node, first, second };
}

export function splitPane(node: PaneNode, paneID: string, direction: SplitDirection): { root: PaneNode; newPaneID: string } {
  const added = newTerminal(findPane(node, paneID)?.cwd ?? '');
  const root = mapPane(node, paneID, (pane) => ({
    kind: 'split',
    id: newID('split'),
    direction,
    ratio: 0.5,
    first: pane,
    second: added,
  }));
  return { root, newPaneID: added.id };
}

export function removePane(node: PaneNode, paneID: string): PaneNode | null {
  if (node.kind === 'terminal') {
    return node.id === paneID ? null : node;
  }
  const first = removePane(node.first, paneID);
  const second = removePane(node.second, paneID);
  if (!first) return second;
  if (!second) return first;
  return first === node.first && second === node.second ? node : { ...node, first, second };
}

export function findPane(node: PaneNode, paneID: string): TerminalLeaf | undefined {
  if (node.kind === 'terminal') return node.id === paneID ? node : undefined;
  return findPane(node.first, paneID) ?? findPane(node.second, paneID);
}

export function paneIDs(node: PaneNode): string[] {
  if (node.kind === 'terminal') return [node.id];
  return [...paneIDs(node.first), ...paneIDs(node.second)];
}

export function updateSplitRatio(node: PaneNode, splitID: string, ratio: number): PaneNode {
  if (node.kind === 'terminal') return node;
  if (node.id === splitID) return { ...node, ratio: Math.min(0.85, Math.max(0.15, ratio)) };
  const first = updateSplitRatio(node.first, splitID, ratio);
  const second = updateSplitRatio(node.second, splitID, ratio);
  return first === node.first && second === node.second ? node : { ...node, first, second };
}
