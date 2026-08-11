import react from "@vitejs/plugin-react";
import { defineConfig, loadEnv } from "vite";

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const siteUrl = (env.SITE_URL ?? "").replace(/\/$/, "");
  const socialImage = siteUrl ? `${siteUrl}/og.png` : "og.png";

  return {
    base: "./",
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
