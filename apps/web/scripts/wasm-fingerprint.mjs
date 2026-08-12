import { createHash } from "node:crypto";
import { readFile, readdir, stat, writeFile } from "node:fs/promises";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const scriptDirectory = dirname(scriptPath);
const webRoot = resolve(scriptDirectory, "..");
const repositoryRoot = resolve(webRoot, "../..");
const fingerprintPath = resolve(webRoot, "public/rusty_spire_wasm.sources.sha256");

const sourceInputs = [
  "Cargo.lock",
  "Cargo.toml",
  "packages/spire-codex-stable-v0.107.1.json",
  "packages/reviewed-effects-v1.json",
  "crates/rusty-spire-api",
  "crates/rusty-spire-combat",
  "crates/rusty-spire-core",
  "crates/rusty-spire-data",
  "crates/rusty-spire-heuristics",
  "crates/rusty-spire-simulator",
  "crates/rusty-spire-wasm/Cargo.toml",
  "crates/rusty-spire-wasm/src",
  "apps/web/scripts/build-wasm.mjs",
  "apps/web/scripts/wasm-fingerprint.mjs",
];

async function collectFiles(path) {
  const metadata = await stat(path);
  if (metadata.isFile()) return [path];
  const entries = await readdir(path, { withFileTypes: true });
  const nested = await Promise.all(
    entries.map((entry) => collectFiles(resolve(path, entry.name))),
  );
  return nested.flat();
}

export async function wasmSourceFingerprint() {
  const paths = (
    await Promise.all(sourceInputs.map((path) => collectFiles(resolve(repositoryRoot, path))))
  )
    .flat()
    .sort((left, right) => left.localeCompare(right));
  const hash = createHash("sha256");
  for (const path of paths) {
    hash.update(relative(repositoryRoot, path));
    hash.update("\0");
    hash.update(await readFile(path));
    hash.update("\0");
  }
  return hash.digest("hex");
}

export async function writeWasmSourceFingerprint() {
  const fingerprint = await wasmSourceFingerprint();
  await writeFile(fingerprintPath, `${fingerprint}\n`, "utf8");
  return fingerprint;
}

export async function checkWasmSourceFingerprint() {
  const expected = await wasmSourceFingerprint();
  const committed = (await readFile(fingerprintPath, "utf8")).trim();
  if (committed !== expected) {
    throw new Error(
      `committed WASM source fingerprint is stale: expected ${expected}, found ${committed}`,
    );
  }
  return expected;
}

if (process.argv[1] && resolve(process.argv[1]) === scriptPath) {
  const command = process.argv[2] ?? "--check";
  if (command === "--write") {
    console.log(await writeWasmSourceFingerprint());
  } else if (command === "--check") {
    console.log(await checkWasmSourceFingerprint());
  } else {
    throw new Error(`unknown command: ${command}`);
  }
}
