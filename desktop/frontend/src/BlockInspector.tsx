import { ClipboardSetText } from '../wailsjs/runtime/runtime';

import { BlockUpdate } from './backend';

type Props = {
  blocks: BlockUpdate[];
  onClose(): void;
  onStatus(message: string, error?: boolean): void;
};

function statusLabel(block: BlockUpdate): string {
  switch (block.status) {
    case 'running': return 'Running';
    case 'cancelled': return 'Cancelled';
    case 'failed': return `Exit ${block.record.exit_code}`;
    case 'completed': return 'Exit 0';
  }
}

function durationLabel(block: BlockUpdate): string {
  const start = Date.parse(block.record.started_at);
  const end = Date.parse(block.record.ended_at);
  if (!Number.isFinite(start) || !Number.isFinite(end) || end < start) return '';
  const duration = end - start;
  if (duration < 1000) return `${duration} ms`;
  return `${(duration / 1000).toFixed(duration < 10000 ? 1 : 0)} s`;
}

export function BlockInspector({ blocks, onClose, onStatus }: Props) {
  const copy = async (label: string, value: string) => {
    try {
      const copied = await ClipboardSetText(value);
      if (!copied) throw new Error('clipboard rejected the copy');
      onStatus(`${label} copied`);
    } catch (error) {
      onStatus(error instanceof Error ? error.message : String(error), true);
    }
  };

  return (
    <aside className="block-inspector" aria-label="Command block evidence">
      <header className="inspector-header">
        <div><span className="inspector-kicker">LOCAL EVIDENCE</span><strong>Command blocks</strong></div>
        <button type="button" aria-label="Close command blocks" onClick={onClose}>×</button>
      </header>
      {blocks.length === 0 ? (
        <div className="blocks-empty">
          <span className="empty-mark">›_</span>
          <strong>No evidence yet</strong>
          <p>Run a command. Its command line, directory, timing, exit state, and output will appear here.</p>
          <code>~/.yarp/history</code>
        </div>
      ) : (
        <div className="block-list" aria-live="polite">
          {blocks.map((block) => {
            const output = block.record.output ?? '';
            const preview = output.length > 3500 ? `${output.slice(0, 3500)}\n…` : output;
            return (
              <article className={`evidence-block ${block.status}`} key={block.id}>
                <header>
                  <span className="block-status">{statusLabel(block)}</span>
                  <span>{durationLabel(block)}</span>
                </header>
                <code className="block-command">{block.record.cmd || '(command unavailable)'}</code>
                {block.record.cwd && <span className="block-cwd">{block.record.cwd}</span>}
                {preview && <pre>{preview}</pre>}
                <footer>
                  <button type="button" onClick={() => { void copy('Command', block.record.cmd); }}>Copy command</button>
                  <button type="button" disabled={!output} onClick={() => { void copy('Output', output); }}>Copy output</button>
                  {block.record.truncated && <span>capture truncated</span>}
                </footer>
              </article>
            );
          })}
        </div>
      )}
    </aside>
  );
}
