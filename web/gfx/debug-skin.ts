import type { Telemetry } from '../audio/protocol';
import type { Controls, Skin } from './skin';

const FLASH_DECAY = 3; // per second

/** Peaks as bars plus a flash per note. */
export class DebugSkin implements Skin {
  private flash = 0;

  note(_index: number, velocity: number): void {
    this.flash = Math.max(this.flash, velocity);
  }

  frame(ctx: CanvasRenderingContext2D, dt: number, t: Telemetry, c: Controls): void {
    const w = ctx.canvas.width;
    const h = ctx.canvas.height;
    ctx.fillStyle = '#000';
    ctx.fillRect(0, 0, w, h);

    if (this.flash > 0) {
      ctx.fillStyle = `rgba(255,255,255,${this.flash.toFixed(3)})`;
      ctx.fillRect(0, 0, w, h);
      this.flash = Math.max(0, this.flash - dt * FLASH_DECAY);
    }

    ctx.fillStyle = '#4af';
    ctx.fillRect(w * 0.25, h * (1 - t.peakL), w * 0.2, h * t.peakL);
    ctx.fillRect(w * 0.55, h * (1 - t.peakR), w * 0.2, h * t.peakR);

    // gesture y grows upward (1 = top), so flip for canvas coordinates
    ctx.fillStyle = '#fa4';
    ctx.fillRect(c.x * w - 2, (1 - c.y) * h - 2, 4, 4);
  }
}
