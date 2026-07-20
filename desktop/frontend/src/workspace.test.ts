import { describe, expect, it } from 'vitest';

import { findPane, newTab, paneIDs, removePane, splitPane, updateSplitRatio } from './workspace';

describe('workspace pane tree', () => {
  it('splits a real leaf without replacing the original pane', () => {
    const tab = newTab('/tmp/project');
    const originalID = tab.focusedPaneID;
    const { root, newPaneID } = splitPane(tab.root, originalID, 'right');

    expect(root.kind).toBe('split');
    expect(paneIDs(root)).toEqual([originalID, newPaneID]);
    expect(findPane(root, newPaneID)?.cwd).toBe('/tmp/project');
  });

  it('collapses a split around the surviving pane', () => {
    const tab = newTab();
    const { root, newPaneID } = splitPane(tab.root, tab.focusedPaneID, 'down');
    const remaining = removePane(root, newPaneID);

    expect(remaining?.kind).toBe('terminal');
    expect(remaining && paneIDs(remaining)).toEqual([tab.focusedPaneID]);
  });

  it('clamps resized split ratios to keep both panes usable', () => {
    const tab = newTab();
    const { root } = splitPane(tab.root, tab.focusedPaneID, 'right');
    if (root.kind !== 'split') throw new Error('expected a split');

    expect(updateSplitRatio(root, root.id, -1)).toMatchObject({ ratio: 0.15 });
    expect(updateSplitRatio(root, root.id, 2)).toMatchObject({ ratio: 0.85 });
  });
});
