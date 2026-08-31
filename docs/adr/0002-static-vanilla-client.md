# Static Astro + hand-written TypeScript for the client

- Status: accepted (2026-08-30)
- M1 decision: yes (the implementation lands in M3)

## Context

Astro's canonical use is SSR/islands, where it ships a Node server. Here the production server is the Axum process, and the client has exactly one page: one list, one add form, three filter buttons. The choice is between (a) a full SSR Astro app (a second Node process, an `@astrojs/node` adapter), and (b) a **static** Astro build plus hand-written TypeScript DOM code.

## Decision

Use **`output: "static"`** with no integrations. The single page is plain HTML plus hand-written TS modules (`lib/api.ts`, `lib/store.ts`, `lib/render.ts`, `client.ts`). In development, `astro dev` runs on `:4321` with HMR and the browser fetches the API on `:3000` directly (CORS on the API). In production the Axum process serves `web/dist` for non-`/api` paths, so there is **one process total** and the client is same-origin.

## Consequences

- **Pro** — no second server process, no Node in production, no adapter; one less whole runtime to run, version, and secure. The API is the only source of truth (there is no SSR fallback), so the client and server cannot drift into serving different data.
- **Pro** — hand-written TS over a 150-line DOM is easier to audit than a component framework would be at this size, and there is nothing to bundle-bloat: the client is a few KB of JS.
- **Con** — no SSR: first paint waits on the initial `fetch`. The client shows a loading state and must handle the API being down (error banner + retry); acceptable for a single-user local tool.
- **Con** — Astro features we *don't* get from static mode (SSR islands, on-demand rendering) are off the table; adopting them later is a deployment-shape change, not a tweak.

## Future readers

If the client ever grows past "one list page," the honest moves are a real frontend framework in the same static Astro shell (record a new ADR) or an SSR Astro app with `@astrojs/node` — which would make Axum-API static serving a development-only mode. Do not quietly add SSR; it changes the process model this whole document describes.
