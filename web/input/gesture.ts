import type { Gesture } from '../audio/protocol';

const clamp01 = (v: number) => (v < 0 ? 0 : v > 1 ? 1 : v);

/**
 * Pixels -> axes in [0,1]; y grows upward (screen top = 1).
 * z is seconds held since down; 0 when not touching. pinch is reserved (0).
 */
export class GestureTracker {
  private downAt: number | null = null;

  constructor(
    private readonly emit: (g: Gesture) => void,
    private readonly now: () => number = () => performance.now(),
  ) {}

  down(x: number, y: number, w: number, h: number): void {
    this.downAt = this.now();
    this.send(x, y, w, h);
  }

  move(x: number, y: number, w: number, h: number): void {
    if (this.downAt === null) return;
    this.send(x, y, w, h);
  }

  up(): void {
    if (this.downAt === null) return;
    this.downAt = null;
    this.last = { ...this.last, z: 0 };
    this.emit(this.last);
  }

  private last: Gesture = { x: 0, y: 0, z: 0, pinch: 0 };

  private send(px: number, py: number, w: number, h: number): void {
    const z = this.downAt === null ? 0 : (this.now() - this.downAt) / 1000;
    this.last = {
      x: w > 0 ? clamp01(px / w) : 0,
      y: h > 0 ? clamp01(1 - py / h) : 0,
      z,
      pinch: 0,
    };
    this.emit(this.last);
  }
}
