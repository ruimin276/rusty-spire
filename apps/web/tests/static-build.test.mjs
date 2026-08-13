import assert from "node:assert/strict";
import { access, readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../", import.meta.url);

test("builds a portable static document", async () => {
  const html = await readFile(new URL("dist/index.html", root), "utf8");
  assert.match(html, /<div id="root">/);
  assert.match(html, /needs a local web server/);
  assert.match(html, /SLS2 Combat Lab — Deterministic combat search/);
  assert.match(html, /content="og\.png"/);
  assert.match(html, /\.\/assets\/[^"']+\.js/);
  assert.match(html, /\.\/assets\/[^"']+\.css/);
  assert.doesNotMatch(html, /__OG_IMAGE__|vinext|wrangler|codex-preview/i);
  await access(new URL("dist/og.png", root));
});

test("ships the simulator contract in the browser bundle", async () => {
  const assetNames = await readdir(new URL("dist/assets/", root));
  const javascript = assetNames.find((name) => name.endsWith(".js"));
  assert.ok(javascript, "expected a JavaScript application bundle");
  const bundle = await readFile(new URL(`dist/assets/${javascript}`, root), "utf8");
  assert.match(bundle, /isolated_combat_xoshiro_v1/);
  assert.match(bundle, /minimize_hp_loss/);
  assert.match(bundle, /https:\/\/github\.com\/ruimin276\/rusty-spire/);
  assert.match(bundle, /Reset starter deck/);
  assert.match(bundle, /Remove one/);
  assert.match(bundle, /Edit full combat state/);
  assert.match(bundle, /Class cards shown by default/);
  assert.match(bundle, /Each character keeps its own deck draft/);
  assert.match(bundle, /All cards/);
  assert.match(bundle, /Potions are not executable yet/);
  assert.doesNotMatch(bundle, /How the solver works|Deterministic combat search/);
  assert.doesNotMatch(bundle, /7a27dc78a49f6523b64dcc140117f8c21690d1fde6240208de488ee0e88e088c/);
  const wasm = await readFile(new URL("dist/rusty_spire_wasm.wasm", root));
  assert.deepEqual([...wasm.subarray(0, 4)], [0, 97, 115, 109]);
  assert.ok(wasm.length > 100_000, "expected the compiled Rust combat engine");
  assert.ok(assetNames.some((name) => name.startsWith("simulator-worker-") && name.endsWith(".js")));
});

test("ships the interactive combat replay", async () => {
  const assetNames = await readdir(new URL("dist/assets/", root));
  const javascript = assetNames.find((name) => name.endsWith(".js"));
  assert.ok(javascript);
  const bundle = await readFile(new URL(`dist/assets/${javascript}`, root), "utf8");
  assert.match(bundle, /Combat replay/);
  assert.match(bundle, /Enemy intent/);
  assert.match(bundle, /Playback speed/);
  assert.match(bundle, /State details and raw trace/);
  assert.match(bundle, /Effective cost/);
  assert.match(bundle, /include_replay/);
  assert.match(bundle, /rusty_spire_wasm\.wasm/);
  assert.match(bundle, /searchParams\.set\([`"]v[`"],/);
});

test("ships the supported Spire Codex artwork", async () => {
  const artwork = [
    "characters/silent.webp",
    "characters/ironclad.webp",
    "monsters/nibbit.webp",
    "monsters/fuzzy_wurm_crawler.webp",
    "monsters/shrinker_beetle.webp",
    "cards/strike_silent.webp",
    "cards/bash.webp",
    "cards/iron_wave.webp",
    "cards/pommel_strike.webp",
    "cards/backflip.webp",
    "cards/adrenaline.webp",
    "cards/ascenders_bane.webp",
    "relics/winged_boots.webp",
  ];
  await Promise.all(artwork.map((path) => access(new URL(`dist/spire-codex/${path}`, root))));
});
