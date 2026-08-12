/// <reference lib="webworker" />

type SimulatorExports = {
  memory: WebAssembly.Memory;
  sls2_alloc(length: number): number;
  sls2_free(pointer: number, length: number): void;
  sls2_call_v1(pointer: number, length: number): bigint;
};

type ApiRequest = { id: number; operation: unknown; wasmUrl: string };
type ApiEnvelope =
  | { ok: true; value: unknown }
  | { ok: false; error: { message: string } | string };

let exportsPromise: Promise<SimulatorExports> | undefined;

async function loadSimulator(wasmUrl: string) {
  if (exportsPromise) return exportsPromise;
  exportsPromise = fetch(wasmUrl)
    .then((response) => {
      if (!response.ok) throw new Error(`cannot load simulator WASM (${response.status})`);
      return response.arrayBuffer();
    })
    .then((bytes) =>
      WebAssembly.instantiate(bytes, { env: { sls2_now_ms: () => performance.now() } }),
    )
    .then(({ instance }) => instance.exports as unknown as SimulatorExports);
  return exportsPromise;
}

async function call(request: ApiRequest) {
  const wasm = await loadSimulator(request.wasmUrl);
  const input = new TextEncoder().encode(JSON.stringify(request.operation));
  const inputPointer = wasm.sls2_alloc(input.length);
  new Uint8Array(wasm.memory.buffer, inputPointer, input.length).set(input);
  let packed: bigint;
  try {
    packed = wasm.sls2_call_v1(inputPointer, input.length);
  } finally {
    wasm.sls2_free(inputPointer, input.length);
  }
  const outputPointer = Number(packed & 0xffff_ffffn);
  const outputLength = Number(packed >> 32n);
  const output = new Uint8Array(wasm.memory.buffer, outputPointer, outputLength).slice();
  wasm.sls2_free(outputPointer, outputLength);
  const envelope = JSON.parse(new TextDecoder().decode(output)) as ApiEnvelope;
  if (!envelope.ok) {
    throw new Error(typeof envelope.error === "string" ? envelope.error : envelope.error.message);
  }
  return envelope.value;
}

self.addEventListener("message", async (event: MessageEvent<ApiRequest>) => {
  try {
    self.postMessage({ id: event.data.id, ok: true, value: await call(event.data) });
  } catch (error) {
    self.postMessage({
      id: event.data.id,
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    });
  }
});
