// ?worker&url makes Vite bundle worklet.ts (with its imports) into a separate JS chunk.
import workletUrl from './worklet.ts?worker&url';
import { encodeCommand, decodeTelemetry, type Command, type FromWorklet, type Telemetry, type ToWorklet } from './protocol';

export class AudioHost {
  onTelemetry: (t: Telemetry) => void = () => {};

  private constructor(private readonly node: AudioWorkletNode) {
    node.port.onmessage = (e: MessageEvent<FromWorklet>) => {
      if (e.data.type === 'telemetry') this.onTelemetry(decodeTelemetry(e.data.buf));
    };
  }

  static async create(ctx: AudioContext, wasmUrl: string, seed: number): Promise<AudioHost> {
    // Compile on the main thread; the Module is structured-cloned to the worklet.
    const module = await WebAssembly.compileStreaming(fetch(wasmUrl));
    await ctx.audioWorklet.addModule(workletUrl);
    const node = new AudioWorkletNode(ctx, 'versum-engine', {
      numberOfInputs: 0,
      outputChannelCount: [2],
      processorOptions: { module, seed },
    });
    return new AudioHost(node);
  }

  /** Commands posted before the worklet is ready are queued on its side. */
  send(c: Command): void {
    this.node.port.postMessage({ type: 'command', cmd: encodeCommand(c) } satisfies ToWorklet);
  }

  connect(dest: AudioNode): void {
    this.node.connect(dest);
  }
}
