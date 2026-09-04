# thaumiel-ui

The admin dashboard: a Next.js app, statically exported, embedded into a single Rust binary that serves it. No Node runtime in production, no separate frontend deployment, no CORS-hostname coordination to get right — one binary, one port.

## How the embedding actually works

`web/` is an ordinary Next.js App Router project, built with `output: "export"` (see `web/next.config.mjs`), which turns the whole app into plain HTML/CSS/JS files under `web/out/`. `build.rs` runs `npm install` (if `node_modules` is missing) and `npm run build` automatically as part of `cargo build -p thaumiel-ui`, before `src/assets.rs`'s `#[derive(RustEmbed)]` scans `web/out/` and bakes every file into the compiled binary as a byte slice. At runtime, `src/serve.rs` answers HTTP requests straight out of that embedded set — no filesystem access, no Node process, nothing to install on the machine actually running the server.

The one thing that can't be baked in at build time is which Thaumiel API this dashboard should point at — that's a per-deployment decision, not a per-build one. So it isn't: the binary serves a small `GET /thaumiel-ui-config.json`, generated at startup from its own loaded config, and the frontend fetches that once on boot (`web/src/lib/runtime-config.ts`) before making any API call. No config file present, no reachable endpoint — it falls back to `http://localhost:8080`, a known-good default for local development, rather than failing to boot.

```
crates/thaumiel-ui/
  build.rs         runs the Next.js build before compiling this crate
  src/
    main.rs         axum server: /thaumiel-ui-config.json + the static file catch-all
    config.rs        UiConfig: bind/port, api.base_url -- layered TOML + env, like thaumiel-config
    assets.rs         RustEmbed over web/out
    serve.rs           request -> embedded file resolution, caching headers
  config/
    default.toml
  web/                the actual Next.js app (not embedded in git -- see .gitignore)
```

## Running it

```bash
cargo run -p thaumiel-ui
```

builds the frontend the first time (needs Node/npm on the machine *building* it — not the machine running the resulting binary) and serves the dashboard on `:4200`, pointed at `http://localhost:8080` by default. Point it elsewhere without touching a file:

```bash
THAUMIEL_UI_API__BASE_URL=https://licenses.example.com cargo run -p thaumiel-ui
```

or by placing an override file at `config/production.toml` next to wherever the binary runs (`THAUMIEL_UI_ENV=production`) — same layering convention as `thaumiel-server` (see the root [`docs/CONFIGURATION.md`](../../docs/CONFIGURATION.md)), except the baseline itself is compiled into the binary rather than read from `config/default.toml` on disk. That's deliberate: `thaumiel-server` and `thaumiel-ui` both default to a relative `config` directory, and if you ever run both from the same working directory, a disk-relative default would risk one silently loading the other's file — their schemas overlap just enough that nothing would complain. An `<env>.toml` on disk is still opt-in and works exactly as you'd expect.

No Node installed on the build machine? `build.rs` detects that, prints a warning instead of failing the whole workspace build, and embeds a small placeholder page that says exactly what's missing. Building the frontend yourself (`cd web && npm install && npm run build`) and rebuilding fixes it; `THAUMIEL_UI_SKIP_WEB_BUILD=1 cargo build -p thaumiel-ui` skips the npm step entirely and reuses whatever's already in `web/out`, for CI setups that build the frontend as a separate cached step.

## Frontend development

The embed-a-static-export approach is what production runs; day-to-day frontend work doesn't need to go through Cargo at all:

```bash
cd web
npm install
npm run dev
```

talks to `next dev`'s own server on `:3000` with hot reload, hitting whichever API `runtime-config.ts` falls back to (`http://localhost:8080` unless a `thaumiel-server` happens to also be running and serving `/thaumiel-ui-config.json`, which it doesn't — that endpoint is `thaumiel-ui`-specific). Run an actual `thaumiel-server` alongside it (`cargo run -p thaumiel-server` from the repo root) to have something real to click against.

## Design notes

AMOLED-dark by design, not just dark mode: `#000000` background, not a dark gray, since that's what actually saves power and reads as "designed for OLED" rather than "dark theme toggle." No gradients anywhere, one flat accent color (a calm blue, used only for active/primary states), 1px borders instead of shadows for separation. The sidebar's expand/collapse is a plain CSS `width` transition — no easing library, no spring physics, just `cubic-bezier(0.4, 0, 0.2, 1)` over 200ms, because a dashboard people live in all day should feel quiet, not like it's demoing itself. All of this lives in `web/src/app/globals.css` (the token palette) and `web/src/components/Sidebar.module.css` (the animation) if it needs adjusting.
