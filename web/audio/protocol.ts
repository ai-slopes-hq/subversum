// Wire layout shared with crates/wasm (repr(C) Telemetry, little-endian).
export const PROTOCOL_VERSION = 1;
export const TELEMETRY_SIZE = 220;
export const MAX_STEPS = 16;

const OFF = {
  version: 0, frame: 4, peakL: 8, peakR: 12, stepCount: 16,
  droppedSteps: 20, droppedCommands: 24, steps: 28,
} as const;
const STEP_SIZE = 12;

export type Gesture = { x: number; y: number; z: number; pinch: number };

/**
 * Seed travels through engine_command's f32 argument: seeds above 2^24 lose
 * precision (known limitation, deferred).
 */
export type Command =
  | ({ kind: 'gesture' } & Gesture)
  | { kind: 'world'; id: number }
  | { kind: 'seed'; seed: number }
  | { kind: 'tempo'; bpm: number }
  | { kind: 'hold'; on: boolean }
  | { kind: 'lock'; on: boolean };

/** [kind, a, b, c, d] as passed to wasm engine_command. */
export type EncodedCommand = [number, number, number, number, number];

export function encodeCommand(c: Command): EncodedCommand {
  switch (c.kind) {
    case 'gesture': return [0, c.x, c.y, c.z, c.pinch];
    case 'world': return [1, c.id, 0, 0, 0];
    case 'seed': return [2, c.seed, 0, 0, 0];
    case 'tempo': return [3, c.bpm, 0, 0, 0];
    case 'hold': return [4, c.on ? 1 : 0, 0, 0, 0];
    case 'lock': return [5, c.on ? 1 : 0, 0, 0, 0];
  }
}

export type Step = { index: number; note: number; velocity: number };

export type Telemetry = {
  version: number;
  /** samples rendered so far */
  frame: number;
  peakL: number;
  peakR: number;
  droppedSteps: number;
  droppedCommands: number;
  steps: Step[];
};

export function decodeTelemetry(buf: ArrayBuffer): Telemetry {
  if (buf.byteLength < TELEMETRY_SIZE) {
    throw new Error(`telemetry too short: ${buf.byteLength} < ${TELEMETRY_SIZE}`);
  }
  const v = new DataView(buf);
  const n = Math.min(v.getUint32(OFF.stepCount, true), MAX_STEPS);
  const steps: Step[] = [];
  for (let i = 0; i < n; i++) {
    const o = OFF.steps + i * STEP_SIZE;
    steps.push({
      index: v.getUint32(o, true),
      note: v.getUint32(o + 4, true),
      velocity: v.getFloat32(o + 8, true),
    });
  }
  return {
    version: v.getUint32(OFF.version, true),
    frame: v.getUint32(OFF.frame, true),
    peakL: v.getFloat32(OFF.peakL, true),
    peakR: v.getFloat32(OFF.peakR, true),
    droppedSteps: v.getUint32(OFF.droppedSteps, true),
    droppedCommands: v.getUint32(OFF.droppedCommands, true),
    steps,
  };
}

// Messages over the worklet port.
export type ToWorklet = { type: 'command'; cmd: EncodedCommand };
export type FromWorklet =
  | { type: 'ready' }
  | { type: 'telemetry'; buf: ArrayBuffer };
