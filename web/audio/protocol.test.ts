import { readFileSync } from 'node:fs';
import { beforeAll, describe, expect, it } from 'vitest';
import { EngineCore, type EngineExports } from './engine-core';
import { decodeTelemetry, encodeCommand, TELEMETRY_SIZE } from './protocol';

const WASM = new URL('../../target/wasm32-unknown-unknown/release/versum_wasm.wasm', import.meta.url);
let module: WebAssembly.Module;
beforeAll(async () => { module = await WebAssembly.compile(readFileSync(WASM)); });

async function engine(): Promise<EngineExports> {
  const inst = await WebAssembly.instantiate(module, {});
  const ex = inst.exports as unknown as EngineExports;
  ex.engine_init(48000, 1);
  return ex;
}
const tel = (ex: EngineExports) => {
  const p = ex.engine_telemetry();
  return decodeTelemetry(ex.memory.buffer.slice(p, p + TELEMETRY_SIZE));
};
const run = (ex: EngineExports, samples: number) => { for (let i = 0; i < samples; i += 128) ex.engine_process(128); };

describe('protocol vs real wasm', () => {
  it('layout_matches_rust', async () => {
    const ex = await engine();
    expect(ex.engine_telemetry_size()).toBe(TELEMETRY_SIZE);
    run(ex, 512);
    const t = tel(ex);
    expect(t.version).toBe(1);
    expect(t.frame).toBe(512);
  });

  it('steps_arrive_at_exact_frame', async () => {
    const ex = await engine();
    ex.engine_command(3, 120, 0, 0, 0);
    run(ex, 6000); // 47 blocks = 6016 samples, the block containing sample 6000
    const t = tel(ex);
    const s = t.steps.find((x) => x.index === 1);
    expect(s).toBeDefined();
    // telemetry has no per-step frame; the step must appear within one block of 6000
    expect(t.frame).toBeLessThanOrEqual(6000 + 128);
  });

  it('step 1 is not yet present one block before sample 6000', async () => {
    const ex = await engine();
    ex.engine_command(3, 120, 0, 0, 0);
    run(ex, 5888);
    expect(tel(ex).steps.find((x) => x.index === 1)).toBeUndefined();
  });

  it('ack_clears_steps', async () => {
    const ex = await engine();
    run(ex, 6000);
    ex.engine_ack();
    const t = tel(ex);
    expect(t.steps.length).toBe(0);
    expect(t.droppedSteps).toBe(0);
  });

  it('encode_command_matches_wasm_kinds', () => {
    expect(encodeCommand({ kind: 'gesture', x: 1, y: 2, z: 3, pinch: 4 })).toEqual([0, 1, 2, 3, 4]);
    expect(encodeCommand({ kind: 'world', id: 2 })).toEqual([1, 2, 0, 0, 0]);
    expect(encodeCommand({ kind: 'seed', seed: 7 })).toEqual([2, 7, 0, 0, 0]);
    expect(encodeCommand({ kind: 'tempo', bpm: 90 })).toEqual([3, 90, 0, 0, 0]);
    expect(encodeCommand({ kind: 'hold', on: true })).toEqual([4, 1, 0, 0, 0]);
    expect(encodeCommand({ kind: 'lock', on: false })).toEqual([5, 0, 0, 0, 0]);
  });
});

describe('EngineCore', () => {
  it('queues commands before start and applies them at start', async () => {
    const inst = await WebAssembly.instantiate(module, {});
    const core = new EngineCore(inst.exports as unknown as EngineExports, 48000, 1);
    core.command(encodeCommand({ kind: 'tempo', bpm: 240 })); // before start
    core.start();
    const l = new Float32Array(128), r = new Float32Array(128);
    let last: ArrayBuffer | null = null;
    for (let i = 0; i < 8 * 8; i++) { const t = core.process(l, r); if (t) last = t; }
    const t = decodeTelemetry(last!);
    expect(t.frame).toBe(64 * 128);
    // 240 bpm => step every 3000 samples; 8192 samples => steps 0..2 were produced across batches
    expect(t.version).toBe(1);
  });

  it('emits telemetry every 8th block and acks', async () => {
    const inst = await WebAssembly.instantiate(module, {});
    const core = new EngineCore(inst.exports as unknown as EngineExports, 48000, 1);
    core.start();
    const l = new Float32Array(128), r = new Float32Array(128);
    const got = Array.from({ length: 16 }, () => core.process(l, r));
    expect(got.map((g) => g !== null)).toEqual(Array.from({ length: 16 }, (_, i) => i % 8 === 7));
    expect(decodeTelemetry(got[7]!).steps.length).toBeGreaterThan(0);
  });
});
