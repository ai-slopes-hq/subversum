import { AudioHost } from '../audio/host';
import { DebugSkin } from '../gfx/debug-skin';
import { GestureTracker } from '../input/gesture';
import { isTap } from '../input/tap';
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

// Only the first finger drives the gesture; other pointers are ignored until it lifts.
let activePointer: number | null = null;
let downAt = 0;
let downX = 0;
let downY = 0;

canvas.addEventListener('pointerdown', (e) => {
  if (activePointer !== null) return;
  activePointer = e.pointerId;
  downAt = performance.now();
  downX = e.clientX;
  downY = e.clientY;
  canvas.setPointerCapture(e.pointerId);
  tracker.down(e.clientX, e.clientY, canvas.clientWidth, canvas.clientHeight);
});
canvas.addEventListener('pointermove', (e) => {
  if (e.pointerId !== activePointer) return;
  tracker.move(e.clientX, e.clientY, canvas.clientWidth, canvas.clientHeight);
});
const release = (e: PointerEvent, cancelled: boolean): void => {
  if (e.pointerId !== activePointer) return;
  activePointer = null;
  // Short, nearly still press = P-tap; a longer press already acted as Z (hold) via the gesture stream.
  const tap = isTap({
    cancelled,
    heldS: (performance.now() - downAt) / 1000,
    movedPx: Math.hypot(e.clientX - downX, e.clientY - downY),
    viewportPx: Math.max(canvas.clientWidth, canvas.clientHeight),
  });
  if (tap) host?.send({ kind: 'ptap' });
  tracker.up();
};
canvas.addEventListener('pointerup', (e) => release(e, false));
canvas.addEventListener('pointercancel', (e) => release(e, true));

const idle: Telemetry = {
  version: 2, frame: 0, peakL: 0, peakR: 0, droppedSteps: 0, droppedCommands: 0, steps: [],
  bpm: 0, root: 0, scale: 0, hue: 0, brightness: 0, activity: 0, gateEnv: 0, pattern: new Array(16).fill(0),
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
  let ctx: AudioContext | null = null;
  try {
    // Created inside the user gesture so iOS Safari allows it to run.
    ctx = new AudioContext();
    await ctx.resume();
    const seedParam = params.get('seed');
    const parsed = seedParam === null || seedParam === '' ? NaN : Number(seedParam);
    const seed = Number.isInteger(parsed) ? parsed >>> 0 : crypto.getRandomValues(new Uint32Array(1))[0];
    const h = await AudioHost.create(ctx, `${import.meta.env.BASE_URL}engine.wasm`, seed);
    h.onTelemetry = (t) => {
      telemetry = t;
      for (const s of t.steps) skin.note(s.index, s.velocity);
    };
    h.onError = () => { msg.textContent = 'audio processor failed (see console)'; };
    // the engine already starts in amber with this seed (Engine::new via processorOptions)
    h.connect(ctx.destination);
    host = h;
    startBtn.remove();
  } catch (err) {
    // Do not leak the context across retries (iOS caps live contexts).
    try { await ctx?.close(); } catch { /* already closed */ }
    msg.textContent = `start failed: ${err instanceof Error ? err.message : String(err)}`;
    startBtn.disabled = false;
  }
});
