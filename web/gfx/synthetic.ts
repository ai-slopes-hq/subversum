import type { Telemetry } from '../audio/protocol';

/** Deterministic fake telemetry: a step every 15 frames, wobbling peaks. */
export function syntheticTelemetry(frame: number): Telemetry {
  const wobble = 0.5 + 0.5 * Math.sin(frame * 0.1);
  const steps = frame % 15 === 0
    ? [{ index: (frame / 15) % 16, note: 36 + ((frame / 15) % 12), velocity: 0.5 + 0.5 * wobble }]
    : [];
  return {
    version: 2,
    frame: frame * 128,
    peakL: wobble,
    peakR: 0.5 * wobble + 0.25,
    droppedSteps: 0,
    droppedCommands: 0,
    steps,
    bpm: 120,
    root: 0,
    scale: 0,
    hue: 0,
    brightness: 0.5,
    activity: wobble,
    gateEnv: wobble,
    pattern: new Array(16).fill(0),
  };
}
