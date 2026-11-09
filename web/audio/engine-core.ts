import { TELEMETRY_SIZE, type EncodedCommand } from './protocol';

export type EngineExports = {
  memory: WebAssembly.Memory;
  engine_init(sr: number, seed: number): void;
  engine_command(kind: number, a: number, b: number, c: number, d: number): void;
  engine_process(frames: number): void;
  engine_out_l(): number;
  engine_out_r(): number;
  engine_telemetry(): number;
  engine_telemetry_size(): number;
  engine_ack(): void;
};

export const BLOCK = 128;
export const TELEMETRY_EVERY_BLOCKS = 8;

/** Plain (non-worklet) logic of the processor: testable in node. */
export class EngineCore {
  private ready = false;
  private queue: EncodedCommand[] = [];
  private blocks = 0;
  private viewBuffer: ArrayBuffer | null = null;
  private viewN = 0;
  private viewL: Float32Array | null = null;
  private viewR: Float32Array | null = null;

  constructor(private readonly ex: EngineExports, private readonly sampleRate: number, private readonly seed: number) {}

  /** Commands before start() are queued and applied at start. */
  command(c: EncodedCommand): void {
    if (!this.ready) { this.queue.push(c); return; }
    this.ex.engine_command(c[0], c[1], c[2], c[3], c[4]);
  }

  start(): void {
    this.ex.engine_init(this.sampleRate, this.seed);
    if (this.ex.engine_telemetry_size() !== TELEMETRY_SIZE) {
      throw new Error(`telemetry layout mismatch: wasm ${this.ex.engine_telemetry_size()} != ${TELEMETRY_SIZE}`);
    }
    this.ready = true;
    for (const c of this.queue) this.command(c);
    this.queue = [];
  }

  /** Renders one block into outL/outR; returns telemetry bytes every 8th block (then acks). */
  process(outL: Float32Array, outR: Float32Array): ArrayBuffer | null {
    if (!this.ready) return null;
    const n = outL.length;
    this.ex.engine_process(n);
    const mem = this.ex.memory.buffer;
    const pl = this.ex.engine_out_l();
    const pr = this.ex.engine_out_r();
    if (!this.viewL || !this.viewR || this.viewBuffer !== mem || this.viewN !== n
        || this.viewL.byteOffset !== pl || this.viewR.byteOffset !== pr) {
      this.viewBuffer = mem;
      this.viewN = n;
      this.viewL = new Float32Array(mem, pl, n);
      this.viewR = new Float32Array(mem, pr, n);
    }
    outL.set(this.viewL);
    outR.set(this.viewR);
    if (++this.blocks % TELEMETRY_EVERY_BLOCKS !== 0) return null;
    const ptr = this.ex.engine_telemetry();
    const copy = mem.slice(ptr, ptr + TELEMETRY_SIZE);
    this.ex.engine_ack();
    return copy;
  }
}
