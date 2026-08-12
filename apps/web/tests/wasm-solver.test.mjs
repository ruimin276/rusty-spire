import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { performance } from "node:perf_hooks";

const root = new URL("../", import.meta.url);
const wasmPath = process.env.RUSTY_SPIRE_WASM_PATH ?? "dist/rusty_spire_wasm.wasm";

test("executes the Rust optimal search through the browser ABI", async () => {
  const [wasmBytes, fixture] = await Promise.all([
    readFile(new URL(wasmPath, root)),
    readFile(new URL("../../fixtures/combat_setup_v1/silent_nibbit_seed_1.json", root), "utf8"),
  ]);
  const { instance } = await WebAssembly.instantiate(wasmBytes, {
    env: { sls2_now_ms: () => performance.now() },
  });
  const wasm = instance.exports;
  const input = new TextEncoder().encode(fixture);
  const inputPointer = wasm.sls2_alloc(input.length);
  new Uint8Array(wasm.memory.buffer, inputPointer, input.length).set(input);
  const packed = wasm.sls2_solve_json(inputPointer, input.length, 100_000, 50, 20_000);
  wasm.sls2_free(inputPointer, input.length);

  const outputPointer = Number(packed & 0xffff_ffffn);
  const outputLength = Number(packed >> 32n);
  const output = new Uint8Array(wasm.memory.buffer, outputPointer, outputLength).slice();
  wasm.sls2_free(outputPointer, outputLength);
  const envelope = JSON.parse(new TextDecoder().decode(output));

  assert.equal(envelope.ok, true, envelope.error);
  assert.equal(envelope.value.result.won, true);
  assert.equal(envelope.value.result.optimality_proven, true);
  assert.equal(envelope.value.result.hp_loss, 0);
  assert.equal(envelope.value.result.final_hp, 70);
  assert.equal(envelope.value.result.actions.length, 20);
  assert.deepEqual(envelope.value.opening_hand, [
    "CARD.DEFEND_SILENT",
    "CARD.STRIKE_SILENT",
    "CARD.STRIKE_SILENT",
    "CARD.SURVIVOR",
    "CARD.DEFEND_SILENT",
    "CARD.STRIKE_SILENT",
    "CARD.STRIKE_SILENT",
  ]);
});

test("serves content through the versioned dispatcher", async () => {
  const wasmBytes = await readFile(new URL(wasmPath, root));
  const { instance } = await WebAssembly.instantiate(wasmBytes, {
    env: { sls2_now_ms: () => performance.now() },
  });
  const wasm = instance.exports;
  const input = new TextEncoder().encode(JSON.stringify({ operation: "content_info" }));
  const pointer = wasm.sls2_alloc(input.length);
  new Uint8Array(wasm.memory.buffer, pointer, input.length).set(input);
  const packed = wasm.sls2_call_v1(pointer, input.length);
  wasm.sls2_free(pointer, input.length);
  const outputPointer = Number(packed & 0xffff_ffffn);
  const outputLength = Number(packed >> 32n);
  const output = new Uint8Array(wasm.memory.buffer, outputPointer, outputLength).slice();
  wasm.sls2_free(outputPointer, outputLength);
  const envelope = JSON.parse(new TextDecoder().decode(output));
  assert.equal(envelope.ok, true);
  assert.equal(envelope.value.package.package_id, "spire-codex-stable-v0.107.1");
  assert.ok(envelope.value.cards.some((card) => card.id === "CARD.ADRENALINE"));
});
