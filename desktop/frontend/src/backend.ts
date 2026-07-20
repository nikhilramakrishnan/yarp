export type SessionInfo = {
  id: string;
  shell: string;
  cwd: string;
};

export type BlockRecord = {
  session: string;
  seq: number;
  cmd: string;
  cwd?: string;
  started_at: string;
  ended_at: string;
  exit_code: number;
  output?: string;
  truncated?: boolean;
};

export type BlockUpdate = {
  id: string;
  revision: number;
  status: 'running' | 'completed' | 'failed' | 'cancelled';
  reason?: string;
  record: BlockRecord;
};

type AppBindings = {
  StartTerminal(cols: number, rows: number, cwd: string): Promise<SessionInfo>;
  WriteTerminal(sessionID: string, data: string): Promise<void>;
  ResizeTerminal(sessionID: string, cols: number, rows: number): Promise<void>;
  AckTerminalOutput(sessionID: string, sequence: number): Promise<void>;
  StopTerminal(sessionID: string): Promise<void>;
  GetRecentBlocks(limit: number): Promise<BlockUpdate[]>;
};

declare global {
  interface Window {
    go?: {
      main?: {
        App?: AppBindings;
      };
    };
  }
}

function bindings(): AppBindings {
  const app = window.go?.main?.App;
  if (!app) {
    throw new Error('The Yarp desktop backend is not available.');
  }
  return app;
}

export function startTerminal(cols: number, rows: number, cwd = ''): Promise<SessionInfo> {
  return bindings().StartTerminal(cols, rows, cwd);
}

export function writeTerminal(sessionID: string, data: string): Promise<void> {
  return bindings().WriteTerminal(sessionID, data);
}

export function resizeTerminal(sessionID: string, cols: number, rows: number): Promise<void> {
  return bindings().ResizeTerminal(sessionID, cols, rows);
}

export function ackTerminalOutput(sessionID: string, sequence: number): Promise<void> {
  return bindings().AckTerminalOutput(sessionID, sequence);
}

export function stopTerminal(sessionID: string): Promise<void> {
  return bindings().StopTerminal(sessionID);
}

export function getRecentBlocks(limit = 100): Promise<BlockUpdate[]> {
  return bindings().GetRecentBlocks(limit);
}
