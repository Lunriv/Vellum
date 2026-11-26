# Vellum

An immersive AI Keeper (AI KP) for single-player investigative Call of Cthulhu (7e subset) tabletop adventures, built with a **Rust-first backend** and a **React / TypeScript frontend**.

> Status: Playable V0.1–V0.5 mock-mode vertical slice (*Crimson Rain Manor*) implemented against frozen baseline `v0.3.1` ([docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)) and [docs/ARCHITECTURE_PLAN.md](docs/ARCHITECTURE_PLAN.md).

## Core Principles

- **Deterministic Truth, Creative Narrative**: Dice rolls, attributes, resources, clues, permissions, progress clocks, and committed world facts are owned exclusively by the deterministic Rust Game Kernel. LLMs propose actions and narrate committed outcomes; they never mutate authoritative state directly.
- **Player Sovereignty**: The player can attempt any reasonable action (including off-rail plans) and retains full ownership over irreversible decisions, Luck spending, and Push the Roll.
- **Dynamic View Protocol (DVP)**: Typed, schema-validated UI components (inline dice, diegetic handouts, evidence board, progress clocks, atmosphere shifts) replace raw chat logs.

## Repository Layout

```text
Vellum/
├── docs/                       # Frozen baseline architecture, Neon implementation plan, rules scope, eval plan
├── modules/crimson_rain_manor/ # Original V1 benchmark scenario (Module IR YAML + static tests)
├── apps/
│   ├── server/                 # Rust backend (Axum + Tokio + SQLx/SQLite + deterministic Game Kernel)
│   └── web/                    # React + TypeScript + Vite + Tailwind CSS + DVP frontend
├── evals/                      # Golden Trace Replay, Agent Regression, Red Team & Playtest Rubric
├── scripts/                    # Mechanical architecture boundary checks
└── .github/workflows/ci.yml    # Automated pre-merge checks (Rust + Web + Architecture Guard)
```

## Quick Start (Local Mock Mode)

Copy `.env.example` to `.env` if you want to customize host, port, SQLite path, or switch `VELLUM_LLM_MODE=live`.

```bash
# 1. Start the Rust backend (http://127.0.0.1:3001, default mock provider)
cargo run --bin vellum-server

# 2. In another terminal, install frontend deps and start Vite (http://127.0.0.1:5173)
pnpm install
pnpm web:dev
```

## Verification Commands

```bash
# Architecture boundary guard
pnpm check:arch

# Backend (apps/server)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Frontend (apps/web)
pnpm web:typecheck
pnpm web:lint
pnpm web:test
pnpm web:build
```

## Key Documents

- [Frozen Architecture Baseline (v0.3.1)](docs/ARCHITECTURE.md)
- [Engineering Architecture & Execution Plan](docs/ARCHITECTURE_PLAN.md)
- [Design Manifesto](docs/DESIGN_MANIFESTO.md)
- [V1 CoC 7e Rules Scope](docs/RULES_SCOPE.md)
- [Evaluation Plan & Hard Gates](docs/EVAL_PLAN.md)
