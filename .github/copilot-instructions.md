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

* Do not deploy for the user (me). Never. Tell me when we need to deploy. 

* we want to keep as much code shared between desktop and wasm as possible.

* tooltips should appear instantly, we might have an utility for this.

## WASM Gotchas

* **egui_plot in WASM**: Plots must always be rendered, even with empty data. Use `PlotPoints::default()` for empty state. Never early-return before showing the plot or it won't render at all in WASM (layout issue).

## Server Access

* **SSH to server**: Use `plink -no-antispoof tatsugo` to run commands on the production server
* **PostgreSQL**: Run `plink -no-antispoof tatsugo "sudo -u postgres psql -d overachiever -c 'YOUR SQL HERE'"` to query the database