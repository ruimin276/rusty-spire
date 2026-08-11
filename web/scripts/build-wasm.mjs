import { spawnSync } from "node:child_process";
import { copyFile, mkdir } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { writeWasmSourceFingerprint } from "./wasm-fingerprint.mjs";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const webRoot = resolve(scriptDirectory, "..");
const repositoryRoot = resolve(webRoot, "..");

const build = spawnSync(
  "cargo",
  ["build", "--release", "--target", "wasm32-unknown-unknown", "-p", "rusty-spire-wasm"],
  { cwd: repositoryRoot, stdio: "inherit" },
);

if (build.error) throw build.error;
if (build.status !== 0) process.exit(build.status ?? 1);

const source = resolve(
  repositoryRoot,
  "target/wasm32-unknown-unknown/release/rusty_spire_wasm.wasm",
);
const destinationDirectory = resolve(webRoot, "public");
await mkdir(destinationDirectory, { recursive: true });
await copyFile(source, resolve(destinationDirectory, "rusty_spire_wasm.wasm"));
await writeWasmSourceFingerprint();
