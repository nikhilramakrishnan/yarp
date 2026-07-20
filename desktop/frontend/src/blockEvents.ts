import { EventsOn } from '../wailsjs/runtime/runtime';

import { BlockUpdate, getRecentBlocks } from './backend';

type Listener = (blocks: BlockUpdate[]) => void;

const updates = new Map<string, BlockUpdate>();
const listeners = new Set<Listener>();

function sortedBlocks(): BlockUpdate[] {
  return Array.from(updates.values()).sort((left, right) => {
    const leftTime = Date.parse(left.record.started_at) || 0;
    const rightTime = Date.parse(right.record.started_at) || 0;
    if (leftTime !== rightTime) return rightTime - leftTime;
    return right.record.seq - left.record.seq;
  });
}

function publish(): void {
  const snapshot = sortedBlocks();
  for (const listener of listeners) listener(snapshot);
}

function apply(update: BlockUpdate, notify = true): void {
  const current = updates.get(update.id);
  if (current && current.revision > update.revision) return;
  updates.set(update.id, update);
  if (notify) publish();
}

EventsOn('block:update', (update: BlockUpdate) => apply(update));

export async function hydrateBlocks(limit = 100): Promise<void> {
  const restored = await getRecentBlocks(limit);
  for (const update of restored) apply(update, false);
  publish();
}

export function subscribeToBlocks(listener: Listener): () => void {
  listeners.add(listener);
  listener(sortedBlocks());
  return () => listeners.delete(listener);
}
