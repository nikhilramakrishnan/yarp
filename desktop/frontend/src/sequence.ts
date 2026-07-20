export type Sequenced<T> = { sequence: number; value: T };

export class ContiguousQueue<T> {
  private next = 1;
  private readonly pending = new Map<number, T>();

  push(sequence: number, value: T): Sequenced<T>[] {
    if (sequence < this.next || this.pending.has(sequence)) return [];
    this.pending.set(sequence, value);
    const ready: Sequenced<T>[] = [];
    while (this.pending.has(this.next)) {
      const nextValue = this.pending.get(this.next);
      if (nextValue === undefined) break;
      this.pending.delete(this.next);
      ready.push({ sequence: this.next, value: nextValue });
      this.next += 1;
    }
    return ready;
  }
}

export class ContiguousProgress {
  private highest = 0;
  private readonly pending = new Set<number>();

  mark(sequence: number): number {
    if (sequence <= this.highest) return this.highest;
    this.pending.add(sequence);
    while (this.pending.delete(this.highest + 1)) this.highest += 1;
    return this.highest;
  }

  value(): number {
    return this.highest;
  }
}
