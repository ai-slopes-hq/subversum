import { describe, expect, it } from 'vitest';
import type { Gesture } from '../audio/protocol';
import { GestureTracker } from './gesture';

function setup() {
  const out: Gesture[] = [];
  let now = 0;
  const g = new GestureTracker((x) => out.push(x), () => now);
  return { g, out, advance: (ms: number) => { now += ms; } };
}

describe('gesture tracker', () => {
  it('gesture_axes_are_normalized_and_clamped', () => {
    const { g, out } = setup();
    g.down(-50, 900, 200, 100);
    g.move(500, -20, 200, 100);
    for (const e of out) {
      for (const v of [e.x, e.y, e.pinch]) {
        expect(v).toBeGreaterThanOrEqual(0);
        expect(v).toBeLessThanOrEqual(1);
      }
      expect(e.z).toBeGreaterThanOrEqual(0);
    }
    expect(out[0]).toMatchObject({ x: 0, y: 0 });
    expect(out[1]).toMatchObject({ x: 1, y: 1 });
  });

  it('gesture_z_counts_only_while_held', () => {
    const { g, out, advance } = setup();
    g.down(10, 10, 100, 100);
    advance(1500);
    g.move(20, 20, 100, 100);
    expect(out[out.length - 1].z).toBeCloseTo(1.5);
    g.up();
    expect(out[out.length - 1].z).toBe(0);
    const n = out.length;
    g.move(30, 30, 100, 100); // no touch: ignored
    expect(out.length).toBe(n);
    g.tick();
    expect(out.length).toBe(n);
  });

  it('gesture_tick_grows_z_while_held_still', () => {
    const { g, out, advance } = setup();
    g.down(10, 10, 100, 100);
    advance(500);
    g.tick();
    const z1 = out[out.length - 1].z;
    expect(z1).toBeGreaterThan(0);
    advance(500);
    g.tick();
    expect(out[out.length - 1].z).toBeGreaterThan(z1);
  });
});
