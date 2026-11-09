import { AudioHost } from '../audio/host';
import { DebugSkin } from '../gfx/debug-skin';
import { GestureTracker } from '../input/gesture';
import type { Telemetry } from '../audio/protocol';
import type { Controls } from '../gfx/skin';

const canvas = document.getElementById('c') as HTMLCanvasElement;
const startBtn = document.getElementById('start') as HTMLButtonElement;
const msg = document.getElementById('msg') as HTMLDivElement;
const tag = document.getElementById('tag') as HTMLDivElement;
const ctx2d = canvas.getContext('2d')!;

const params = new URLSearchParams(location.search);
tag.textContent = `v=${params.get('v') ?? '?'}`;

const skin = new DebugSkin();
let telemetry: Telemetry | null = null;
let controls: Controls = { x: 0, y: 0, z: 0, pinch: 0 };
let host: AudioHost | null = null;

function resize(): void {
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  canvas.width = Math.round(canvas.clientWidth * dpr);
  canvas.height = Math.round(canvas.clientHeight * dpr);
}
window.addEventListener('resize', resize);
resize();

const tracker = new GestureTracker((g) => {
  controls = g;
  host?.send({ kind: 'gesture', ...g });
});

canvas.addEventListener('pointerdown', (e) => {
  canvas.setPointerCapture(e.pointerId);
  tracker.down(e.clientX, e.clientY, canvas.clientWidth, canvas.clientHeight);
});
canvas.addEventListener('pointermove', (e) => tracker.move(e.clientX, e.clientY, canvas.clientWidth, canvas.clientHeight));
canvas.addEventListener('pointerup', () => tracker.up());
canvas.addEventListener('pointercancel', () => tracker.up());

const idle: Telemetry = {
  version: 1, frame: 0, peakL: 0, peakR: 0, droppedSteps: 0, droppedCommands: 0, steps: [],
};

let last = performance.now();
function loop(now: number): void {
  const dt = Math.min((now - last) / 1000, 0.1);
  last = now;
  tracker.tick(); // grows z while a finger is held still
  skin.frame(ctx2d, dt, telemetry ?? idle, controls);
  requestAnimationFrame(loop);
}
requestAnimationFrame(loop);

startBtn.addEventListener('click', async () => {
  startBtn.disabled = true;
  try {
    // Created inside the user gesture so iOS Safari allows it to run.
    const ctx = new AudioContext();
    await ctx.resume();
    const seed = Number(params.get('seed')) || (Math.random() * 0xffffff) | 0;
    const h = await AudioHost.create(ctx, `${import.meta.env.BASE_URL}engine.wasm`, seed);
    h.onTelemetry = (t) => {
      telemetry = t;
      for (const s of t.steps) skin.note(s.index, s.velocity);
    };
    h.onError = () => { msg.textContent = 'audio processor failed (see console)'; };
    h.connect(ctx.destination);
    host = h;
    startBtn.remove();
  } catch (err) {
    msg.textContent = `start failed: ${err instanceof Error ? err.message : String(err)}`;
    startBtn.disabled = false;
  }
});
