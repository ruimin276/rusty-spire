import react from "@vitejs/plugin-react";
import { defineConfig, loadEnv } from "vite";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const siteUrl = (env.SITE_URL ?? "").replace(/\/$/, "");
  const socialImage = siteUrl ? `${siteUrl}/og.png` : "og.png";
  const wasmFingerprint = readFileSync(
    resolve(process.cwd(), "public/rusty_spire_wasm.sources.sha256"),
    "utf8",
  ).trim();

  return {
    base: "./",
    define: {
      __RUSTY_SPIRE_WASM_FINGERPRINT__: JSON.stringify(wasmFingerprint),
    },
    plugins: [
      react(),
      {
        name: "portable-social-metadata",
        transformIndexHtml(html) {
          return html.replaceAll("__OG_IMAGE__", socialImage);
        },
      },
    ],
  };
});
