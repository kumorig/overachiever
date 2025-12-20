# Overachiever - AI Coding Instructions

## Architecture Overview

**Multi-target Rust workspace** with 4 crates sharing `overachiever-core`:

| Crate | Purpose | Database | UI Framework |
|-------|---------|----------|--------------|
| `desktop` | Native Windows app | SQLite (rusqlite) | eframe/egui |
| `wasm` | Web frontend | None (via WebSocket) | eframe/egui (glow) |
| `backend` | Server (REST + WebSocket) | PostgreSQL (tokio-postgres) | N/A |
| `core` | Shared types & messages | N/A | N/A |

## Project Conventions

* Dear Copilot Agent: Do not prompt the user (me) to run command: "npm run deploy",  then read the output and then generate a response. Instead skip to making the response Immediately and only notify the user that we can deploy now. (You are eating my tokens, stop it!)

* we want to keep as much code shared between desktop and wasm as possible, so avoid platform-specific code in core. If needed, use `cfg(target_arch = "wasm32")` or similar.