export type CombatAction = {
  id: string;
  type: string;
  card_id: string | null;
  combat_card_index: string | null;
  target_combat_id: string | null;
  cost: number | null;
  choice_id: string | null;
  selection: string[];
};

export type TraceStep = {
  action: CombatAction;
  state_hash: string;
  hp_loss: number;
};

export type SolveResult = {
  catalog_sha256: string;
  catalog_game_version: string;
  setup_hash: string;
  policy: "minimize_hp_loss";
  won: boolean;
  complete: boolean;
  optimality_proven: boolean;
  hp_loss: number | null;
  final_hp: number | null;
  actions: TraceStep[];
  action_ids: string[];
  explored_states: number;
  cache_hits: number;
  runtime_seconds: number;
  termination_reason: string;
};

export type BrowserSolveResult = {
  result: SolveResult;
  opening_hand: string[];
};

type SolveLimits = {
  maxStates?: number;
  maxTurns?: number;
  timeoutMilliseconds?: number;
};

type WorkerReply =
  | { id: number; ok: true; value: BrowserSolveResult }
  | { id: number; ok: false; error: string };

let worker: Worker | undefined;
let nextRequestId = 1;
const pending = new Map<
  number,
  { resolve: (value: BrowserSolveResult) => void; reject: (error: Error) => void }
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

export function solveCombat(
  setup: unknown,
  { maxStates = 100_000, maxTurns = 50, timeoutMilliseconds = 20_000 }: SolveLimits = {},
) {
  const id = nextRequestId++;
  const request = new Promise<BrowserSolveResult>((resolve, reject) => {
    pending.set(id, { resolve, reject });
  });
  simulatorWorker().postMessage({
    id,
    setup,
    maxStates,
    maxTurns,
    timeoutMilliseconds,
    wasmUrl: new URL("rusty_spire_wasm.wasm", document.baseURI).href,
  });
  return request;
}
