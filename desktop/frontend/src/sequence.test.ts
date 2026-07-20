import { describe, expect, it } from 'vitest';

import { ContiguousProgress, ContiguousQueue } from './sequence';

describe('terminal output sequencing', () => {
  it('holds later batches until the missing sequence arrives', () => {
    const queue = new ContiguousQueue<string>();
    expect(queue.push(2, 'second')).toEqual([]);
    expect(queue.push(1, 'first')).toEqual([
      { sequence: 1, value: 'first' },
      { sequence: 2, value: 'second' },
    ]);
  });

  it('ignores duplicate and already-applied batches', () => {
    const queue = new ContiguousQueue<string>();
    expect(queue.push(1, 'first')).toHaveLength(1);
    expect(queue.push(1, 'duplicate')).toEqual([]);
    expect(queue.push(3, 'third')).toEqual([]);
    expect(queue.push(3, 'duplicate-third')).toEqual([]);
    expect(queue.push(2, 'second')).toEqual([
      { sequence: 2, value: 'second' },
      { sequence: 3, value: 'third' },
    ]);
  });

  it('advances acknowledgements only across rendered contiguous data', () => {
    const progress = new ContiguousProgress();
    expect(progress.mark(2)).toBe(0);
    expect(progress.mark(1)).toBe(2);
    expect(progress.mark(4)).toBe(2);
    expect(progress.mark(3)).toBe(4);
    expect(progress.mark(3)).toBe(4);
  });
});
