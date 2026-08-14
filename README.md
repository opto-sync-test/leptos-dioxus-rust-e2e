# Leptos + Dioxus Rust OptoSync E2E

This fixture proves that one Rust full-stack application can render a Leptos
server shell, embed a Dioxus cross-platform island, and use the official
OptoSync Rust client without copying its reconciliation logic.

The runtime contract is deliberately platform-shaped:

- the web service worker persists writes in IndexedDB before acknowledging
  them, freezes one ordered retry snapshot, and sends it concurrently through
  upload and realtime lanes;
- failed or partial multiplex flushes retain the exact batch for Background
  Sync, Periodic Background Sync, an explicit wake, or an online wake;
- Tokio workers use the same immutable envelope and concurrent lanes that a
  Dioxus mobile or desktop host can invoke from its native scheduler;
- the server accepts both lanes and returns the accepted mutation identities;
- the upstream Rust SDK is included as a pinned recursive Git submodule and is
  exercised by the tests.

## Run locally

```sh
git submodule update --init --recursive
cargo test --all-targets
cargo run
```

Open <http://127.0.0.1:3000>. The CI suite also checks formatting, Clippy,
JavaScript syntax, native tests, and release compilation.

The browser APIs used here are progressive: Background Sync and Periodic
Background Sync are registered when available, while online and app-driven wake
messages provide the portable fallback. Native Dioxus hosts are expected to
invoke `run_background_cycle` from their OS background scheduler.

