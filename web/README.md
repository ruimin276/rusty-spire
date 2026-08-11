# SLS2 Combat Lab static site

This is a portable static React interface for the isolated combat simulator.
The same `rusty-spire-core` initialization and optimal search used by the CLI is
compiled to WebAssembly and executed in a client-side Web Worker. The host only
serves static files; combat inputs and results remain in the visitor's browser.

JavaScript owns the interface and the small JSON/linear-memory ABI. It does not
reimplement or approximate combat mechanics. The embedded catalog is the exact
reviewed catalog used when the WebAssembly module was built.

## Develop and build

```bash
rustup target add wasm32-unknown-unknown
npm install
npm run dev
npm test
```

The production bundle is written to `dist/`. Upload that directory to any
static host, including a personal server, GitHub Pages, Netlify, Cloudflare
Pages, or an object-storage website. All application asset URLs are relative,
so the site also works under a subdirectory.

`npm run build` first compiles `rusty-spire-wasm`, copies the resulting module
into the static assets, then builds the React application. Search has explicit
state, turn, and wall-clock limits. Incomplete searches remain labeled as
incomplete and never claim optimality.

For an absolute social-preview image URL, copy `.env.example` to `.env` and set
`SITE_URL` to the final public URL before running `npm run build`. No server
runtime, worker, database, login, or hosted environment variables are required.
