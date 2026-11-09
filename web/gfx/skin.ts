import type { Telemetry } from '../audio/protocol';

/** Normalized controls, all in [0,1] except z (seconds held). */
export interface Controls { x: number; y: number; z: number; pinch: number }

/** A skin only draws; it never touches audio. */
export interface Skin {
  frame(ctx: CanvasRenderingContext2D, dt: number, t: Telemetry, c: Controls): void;
  note(index: number, velocity: number): void;
}
