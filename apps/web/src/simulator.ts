import type { BrowserSolveResult, CombatSetupV2, ContentManifest } from "./contracts.generated";

export type * from "./contracts.generated";

type SolveLimits = { maxStates?: number; maxTurns?: number; timeoutMilliseconds?: number };
type WorkerReply =
  | { id: number; ok: true; value: unknown }
  | { id: number; ok: false; error: string };

let worker: Worker | undefined;
let nextRequestId = 1;
const pending = new Map<
  number,
  { resolve: (value: unknown) => void; reject: (error: Error) => void }
>();

function simulatorWorker() {
  if (worker) return worker;
  worker = new Worker(new URL("./simulator-worker.ts", import.meta.url), { type: "module" });
  worker.addEventListener("message", (event: MessageEvent<WorkerReply>) => {
    const request = pending.get(event.data.id);
    if (!request) return;
    pending.delete(event.data.id);
    if (event.data.ok) request.resolve(event.data.value);
    else request.reject(new Error(event.data.error));
  });
  worker.addEventListener("error", (event) => {
    const error = new Error(event.message || "the simulator worker stopped unexpectedly");
    for (const request of pending.values()) request.reject(error);
    pending.clear();
    worker?.terminate();
    worker = undefined;
  });
  return worker;
}

function callSimulator<T>(operation: unknown): Promise<T> {
  const id = nextRequestId++;
  const request = new Promise<T>((resolve, reject) => {
    pending.set(id, { resolve: resolve as (value: unknown) => void, reject });
  });
  simulatorWorker().postMessage({
    id,
    operation,
    wasmUrl: new URL("rusty_spire_wasm.wasm", document.baseURI).href,
  });
  return request;
}

export function getContentManifest() {
  return callSimulator<ContentManifest>({ operation: "content_info" });
}

export function solveCombat(
  setup: CombatSetupV2,
  { maxStates = 100_000, maxTurns = 50, timeoutMilliseconds = 20_000 }: SolveLimits = {},
) {
  return callSimulator<BrowserSolveResult>({
    operation: "solve",
    request: {
      schema_version: 1,
      setup,
      policy: "minimize_hp_loss",
      mode: "exact",
      heuristic: "zero",
      limits: {
        max_states: maxStates,
        max_turns: maxTurns,
        timeout_seconds: timeoutMilliseconds / 1_000,
      },
    },
  });
}
