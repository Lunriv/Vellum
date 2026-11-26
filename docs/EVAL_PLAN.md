# Vellum Evaluation Plan & Hard Gates

> Synchronized with [ARCHITECTURE.md](ARCHITECTURE.md) v0.3.1 (Section 14 & Section 20).

## 1. Three-Layer Evaluation Model

1. **Deterministic Rules & State Verification (CI Mandatory)**
   - Rust unit tests + `proptest` invariants for dice, difficulty tiers, bonus/penalty dice, Luck, Push state machine, SAN, clocks, predicates, and Off-Rail patch validation.
   - SQLite integration tests for transactions, short locks, `session_version` CAS, `Idempotency-Key` replay/reuse, Recovery Lease CAS, and SSE multi-subscriber catch-up.
   - Golden Trace Replay (`evals/replay/`): fixed module version + fixed player inputs + fixed RNG seed -> exact expected event stream & final snapshot.
2. **Agent Behavior & Information Security Regression (`evals/regression/`, `evals/red_team/`)**
   - 14 Red Team personas validating zero unauthorized Protected Truth leakage, zero cross-scene NPC omniscience, Bluff separation from Canonical Facts, DVP timing discipline, and no AI takeover of player sovereignty.
3. **Real Player Experience Playtest (`evals/playtest_rubric.md`)**
   - 3–5 real players completing *Crimson Rain Manor*, scored on initial hypothesis weights: **50% Player Experience / 30% World Trustworthiness / 20% Runtime Performance & Cost**.
   - Hard gates cannot be offset by high prose scores.

## 2. Twenty-One Hard Gates (Section 14.3 Mapping)

| # | Hard Gate | Automated Verification Target | Stage |
|---|---|---|---|
| 1 | Deterministic rules test pass rate = 100% | `cargo test kernel::rules` + `proptest` | V0.1 |
| 2 | Committed stats & state changes match Kernel rules 100% | Reducer + Golden Trace Replay | V0.1 |
| 3 | Duplicate Pending Interrupt resolve produces zero duplicate effects | SQLite idempotency + interrupt tests | V0.2 |
| 4 | Page refresh / reconnect never re-rolls dice or double-charges resources | Persistence + E2E refresh test | V0.2 / V0.5 |
| 5 | Double-clicked `POST /turns` with distinct key returns fast `409` without queueing behind LLM | Axum concurrency test (mock slow LLM) | V0.3 |
| 6 | Same `Idempotency-Key` + same body returns original response; different body returns `409 IDEMPOTENCY_KEY_REUSED` | API idempotency test suite | V0.2 / V0.3 |
| 7 | Multiple concurrent SSE subscribers lose zero events | `ActiveTurnBuffer` fan-out test | V0.3 |
| 8 | Events emitted before SSE attach are replayed via Buffer Catch-up | Late-subscriber `Last-Event-ID` test | V0.3 |
| 9 | Expired cursor triggers `stream_reset` with snapshot, never silent drop | Bounded ring-buffer eviction test | V0.3 |
| 10 | Concurrent Narration Recovery triggers yield exactly one `RecoveryLease` winner | SQLite CAS concurrency test | V0.3 |
| 11 | Recovery emits `narration_reset` + incremented `generationId`; frontend clears partial old text | Backend recovery + Web store unit/E2E test | V0.3 / V0.5 |
| 12 | Persisted `after_narration` directives survive refresh and recovery, rendering without replaying first-reveal animations | Web DVP queue + Playwright reload test | V0.5 |
| 13 | Known red-team suite has 0% Protected Truth leakage | Red-team regression harness | V0.4 / V1 |
| 14 | `keeper_hidden` Progress Clocks never appear in client REST/SSE payloads | Client-visible projection guard test | V0.3 / V0.4 |
| 15 | Every authoritative state mutation traces to a valid committed `GameEvent` | Snapshot-vs-event-replay invariant test | V0.2 |
| 16 | Historical narration reads persisted Transcript, never re-invokes LLM | Turn read / reload spy test | V0.3 |
| 17 | NPCs never reference facts lacking valid provenance | Context assembler filter tests | V0.4 |
| 18 | Unverified player assertions (`Bluff`) never enter `KnowledgeAcquisition` / Canonical Facts | Bluff isolation test | V0.4 |
| 19 | NPCs only receive Transcript entries matching `scene_id` + `audible/visible_entity_ids` or valid provenance | Transcript filter unit tests | V0.4 |
| 20 | Core Handouts / clues / clocks never surface before `narration_complete` (or player typewriter skip) | DVP `after_narration` sequencing test | V0.5 |
| 21 | Case Debrief never fabricates unoccurred history | Debrief event-traceability validation test | V1 |

## 3. Required Telemetry Metrics (Section 14.7)
- `submit_to_first_feedback_ms`
- `proposal_latency_ms`
- `kernel_latency_ms`
- `commit_to_first_narration_token_ms`
- `total_turn_latency_ms`
- Input/output token counts, estimated per-turn cost, recovery duration, SSE reconnect rate, model retry/failure counts, and server CPU/memory footprint.
