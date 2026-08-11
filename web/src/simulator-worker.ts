/// <reference lib="webworker" />

import type { BrowserSolveResult } from "./simulator";

type SimulatorExports = {
  memory: WebAssembly.Memory;
  sls2_alloc(length: number): number;
  sls2_free(pointer: number, length: number): void;
  sls2_solve_json(
    pointer: number,
    length: number,
    maxStates: number,
    maxTurns: number,
    timeoutMilliseconds: number,
  ): bigint;
};

type SolveRequest = {
  id: number;
  setup: unknown;
  maxStates: number;
  maxTurns: number;
  timeoutMilliseconds: number;
  wasmUrl: string;
};

type WasmEnvelope =
  | { ok: true; value: BrowserSolveResult }
  | { ok: false; error: string };

let exportsPromise: Promise<SimulatorExports> | undefined;

async function loadSimulator(wasmUrl: string) {
  if (exportsPromise) return exportsPromise;
  exportsPromise = fetch(wasmUrl)
    .then((response) => {
      if (!response.ok) throw new Error(`cannot load simulator WASM (${response.status})`);
      return response.arrayBuffer();
    })
    .then((bytes) =>
      WebAssembly.instantiate(bytes, {
        env: { sls2_now_ms: () => performance.now() },
      }),
    )
    .then(({ instance }) => instance.exports as unknown as SimulatorExports);
  return exportsPromise;
}

async function solve(request: SolveRequest) {
  const wasm = await loadSimulator(request.wasmUrl);
  const input = new TextEncoder().encode(JSON.stringify(request.setup));
  const inputPointer = wasm.sls2_alloc(input.length);
  new Uint8Array(wasm.memory.buffer, inputPointer, input.length).set(input);

  let packed: bigint;
  try {
    packed = wasm.sls2_solve_json(
      inputPointer,
      input.length,
      request.maxStates,
      request.maxTurns,
      request.timeoutMilliseconds,
    );
  } finally {
    wasm.sls2_free(inputPointer, input.length);
  }

  const outputPointer = Number(packed & 0xffff_ffffn);
  const outputLength = Number(packed >> 32n);
  const output = new Uint8Array(wasm.memory.buffer, outputPointer, outputLength).slice();
  wasm.sls2_free(outputPointer, outputLength);
  const envelope = JSON.parse(new TextDecoder().decode(output)) as WasmEnvelope;
  if (!envelope.ok) throw new Error(envelope.error);
  return envelope.value;
}

self.addEventListener("message", async (event: MessageEvent<SolveRequest>) => {
  try {
    const value = await solve(event.data);
    self.postMessage({ id: event.data.id, ok: true, value });
  } catch (error) {
    self.postMessage({
      id: event.data.id,
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    });
  }
});
