/// <reference types="@types/audioworklet" />
import { EngineCore, type EngineExports } from './engine-core';
import type { FromWorklet, ToWorklet } from './protocol';

class VersumEngine extends AudioWorkletProcessor {
  private core: EngineCore | null = null;

  constructor(options: AudioWorkletNodeOptions) {
    super();
    const { module, seed } = options.processorOptions as { module: WebAssembly.Module; seed: number };
    this.port.onmessage = (e: MessageEvent<ToWorklet>) => {
      this.core?.command(e.data.cmd);
    };
    // Engine has zero imports: instantiate synchronously from the compiled module.
    const inst = new WebAssembly.Instance(module, {});
    const core = new EngineCore(inst.exports as unknown as EngineExports, sampleRate, seed);
    core.start();
    this.core = core;
    this.port.postMessage({ type: 'ready' } satisfies FromWorklet);
  }

  process(_inputs: Float32Array[][], outputs: Float32Array[][]): boolean {
    const out = outputs[0];
    if (!this.core || out.length < 1) return true;
    const t = this.core.process(out[0], out[1] ?? out[0]);
    if (t) this.port.postMessage({ type: 'telemetry', buf: t } satisfies FromWorklet, [t]);
    return true;
  }
}

registerProcessor('versum-engine', VersumEngine);
