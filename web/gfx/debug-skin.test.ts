import { describe, expect, it } from 'vitest';
import { DebugSkin } from './debug-skin';
import { syntheticTelemetry } from './synthetic';

function fakeCtx() {
  const calls: string[] = [];
  const ctx = new Proxy({} as Record<string, unknown>, {
    get: (t, k: string) => {
      if (k in t) return t[k];
      if (k === 'canvas') return { width: 320, height: 240 };
      return (..._a: unknown[]) => { calls.push(k); };
    },
    set: (t, k: string, v) => { t[k] = v; return true; },
  });
  return { ctx: ctx as unknown as CanvasRenderingContext2D, calls };
}

describe('debug skin', () => {
  it('debug_skin_runs_on_synthetic_telemetry_without_audio', () => {
    const { ctx, calls } = fakeCtx();
    const skin = new DebugSkin();
    for (let f = 0; f < 600; f++) {
      const t = syntheticTelemetry(f);
      for (const s of t.steps) skin.note(s.index, s.velocity);
      skin.frame(ctx, 1 / 60, t, { x: 0.5, y: 0.5, z: 0, pinch: 0 });
    }
    expect(calls.length).toBeGreaterThan(0);
    expect(calls).toContain('fillRect');
  });

  it('synthetic_telemetry_is_deterministic_and_has_steps', () => {
    expect(syntheticTelemetry(7)).toEqual(syntheticTelemetry(7));
    expect(Array.from({ length: 120 }, (_, i) => syntheticTelemetry(i)).some((t) => t.steps.length > 0)).toBe(true);
  });
});
