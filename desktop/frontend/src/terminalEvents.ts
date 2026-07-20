import { EventsOn } from '../wailsjs/runtime/runtime';

export type TerminalExit = {
  code: number;
  message: string;
  requested: boolean;
  finalSequence: number;
};

export type TerminalData = {
  encoded: string;
  sequence: number;
};

type Sink = {
  data(event: TerminalData): void;
  exit(event: TerminalExit): void;
  cwd(cwd: string): void;
};

const sinks = new Map<string, Sink>();
const queuedData = new Map<string, TerminalData[]>();
const queuedExit = new Map<string, TerminalExit>();
const queuedCWD = new Map<string, string>();
const forgotten = new Set<string>();

EventsOn('terminal:data', (
  payloadOrSessionID: { sessionID: string; sequence: number; dataBase64: string } | string,
  sequenceOrData?: number | string,
  data?: string,
) => {
  const sessionID = typeof payloadOrSessionID === 'string' ? payloadOrSessionID : payloadOrSessionID.sessionID;
  if (forgotten.has(sessionID)) return;
  const event: TerminalData = typeof payloadOrSessionID === 'object'
    ? { sequence: payloadOrSessionID.sequence, encoded: payloadOrSessionID.dataBase64 }
    : typeof sequenceOrData === 'number'
      ? { sequence: sequenceOrData, encoded: data ?? '' }
      : { sequence: 0, encoded: sequenceOrData ?? '' };
  const sink = sinks.get(sessionID);
  if (sink) {
    sink.data(event);
    return;
  }
  const queue = queuedData.get(sessionID) ?? [];
  queue.push(event);
  queuedData.set(sessionID, queue);
});

EventsOn('terminal:exit', (
  payloadOrSessionID: { sessionID: string; exitCode: number; message: string; requested: boolean; finalSequence: number } | string,
  code?: number,
  message?: string,
  requested = false,
  finalSequence = 0,
) => {
  const sessionID = typeof payloadOrSessionID === 'string' ? payloadOrSessionID : payloadOrSessionID.sessionID;
  if (forgotten.has(sessionID)) return;
  const event = typeof payloadOrSessionID === 'object'
    ? {
        code: payloadOrSessionID.exitCode,
        message: payloadOrSessionID.message,
        requested: payloadOrSessionID.requested,
        finalSequence: payloadOrSessionID.finalSequence,
      }
    : { code: code ?? -1, message: message ?? '', requested, finalSequence };
  const sink = sinks.get(sessionID);
  if (sink) {
    sink.exit(event);
    return;
  }
  queuedExit.set(sessionID, event);
});

EventsOn('terminal:cwd', (payloadOrSessionID: { sessionID: string; cwd: string } | string, cwd?: string) => {
  const sessionID = typeof payloadOrSessionID === 'string' ? payloadOrSessionID : payloadOrSessionID.sessionID;
  if (forgotten.has(sessionID)) return;
  const nextCWD = typeof payloadOrSessionID === 'string' ? cwd ?? '' : payloadOrSessionID.cwd;
  const sink = sinks.get(sessionID);
  if (sink) {
    sink.cwd(nextCWD);
    return;
  }
  queuedCWD.set(sessionID, nextCWD);
});

export function subscribeToTerminal(sessionID: string, sink: Sink): () => void {
  forgotten.delete(sessionID);
  sinks.set(sessionID, sink);
  for (const event of queuedData.get(sessionID) ?? []) sink.data(event);
  queuedData.delete(sessionID);
  const exit = queuedExit.get(sessionID);
  if (exit) sink.exit(exit);
  queuedExit.delete(sessionID);
  const cwd = queuedCWD.get(sessionID);
  if (cwd) sink.cwd(cwd);
  queuedCWD.delete(sessionID);

  return () => {
    if (sinks.get(sessionID) === sink) sinks.delete(sessionID);
    queuedData.delete(sessionID);
    queuedExit.delete(sessionID);
    queuedCWD.delete(sessionID);
  };
}

export function forgetTerminal(sessionID: string): void {
  sinks.delete(sessionID);
  queuedData.delete(sessionID);
  queuedExit.delete(sessionID);
  queuedCWD.delete(sessionID);
  forgotten.add(sessionID);
}

export function decodeBase64(encoded: string): Uint8Array {
  const binary = atob(encoded);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}
