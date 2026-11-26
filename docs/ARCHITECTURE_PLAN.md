# Vellum 工程架构与分阶段实施计划

> **作者**：Gemini 4 Neon（架构设计与主控）  
> **实施基线**：[docs/ARCHITECTURE.md](ARCHITECTURE.md) v0.3.1（冻结基线）  
> **规则子集**：[docs/RULES_SCOPE.md](RULES_SCOPE.md)  
> **评测门禁**：[docs/EVAL_PLAN.md](EVAL_PLAN.md)  
> **协作分工**：Neon（架构/契约/视觉定调/验收） · `gpt-6.2-sol high`（Rust 后端） · `Carbon high`（React 前端）

---

## 1. 架构目标与不可违反红线

Vellum 是一个单人 CoC 7e 调查向 AI Keeper Web 游戏，首个完整可玩模组为原创短模组**《绯雨庄园》（Crimson Rain Manor）**。工程上坚持单进程 Rust 后端 + 单 SQLite 文件 + 单 React/Vite 前端的极简边界，**绝不为尚不存在的规模引入微服务、LangGraph、Redis、Neo4j、FAISS、Celery 或 PostgreSQL**。

### 核心不变式（架构机械门禁 + 测试双重兜底）
1. **确定性内核独占权威状态**：只有 `apps/server/src/kernel/` 下的纯 Rust 代码能够决定骰点、检定等级、幸运扣除、Push 资格、HP/MP/SAN/物品/线索/进度钟变化与 `GameEvent` 提交；LLM 只能提出结构化提案或在状态提交后生成叙事。
2. **裁定前与裁定后严格分离**：除只读低风险快通道 `Path A` 外，所有 `Path B / C` 行动必须先完成 `ActionProposal -> Kernel Validation -> (PendingInterrupt?) -> Atomic SQLite Commit`，再启动 `Narration Stream`。
3. **短锁与快速失败**：每个 Session 仅持毫秒级进程内短临界区锁（检查活动 Turn、校验幂等键、登记 Turn/Interrupt、提交短事务），**严禁**跨越 LLM 网络请求、SSE 推送或玩家等待持锁；已有活动 IC Turn 或 Pending Interrupt 时，新 `POST /turns` 立即返回 `409`。
4. **纵深信息隔离**：`raw_truth` 与 `keeper_hidden` 进度钟绝不进入普通 NPC Prompt、前端 REST 响应或 SSE `state_delta`；NPC 只能接收基于 `scene_id` / `present_entity_ids` / `audible_entity_ids` / `visible_entity_ids` 和 `KnowledgeAcquisition` 过滤后的历史；玩家诈唬（Bluff）只能标记为 `player_assertion_unverified` 并与规范事实物理分离。
5. **DVP 呈现与权限解耦**：所有动态视图经白名单组件注册表渲染，禁止任意 HTML/JS；核心 Handout/线索/进度钟指令在状态提交事务中持久化，默认 `display_timing = after_narration`，前端队列绑定 `(turn_id, narration_generation)` 并在刷新/恢复后幂等还原。

---

## 2. Monorepo 目录与模块依赖边界

```text
Vellum/
├── docs/
│   ├── ARCHITECTURE.md            # v0.3.1 冻结基线原文
│   ├── ARCHITECTURE_PLAN.md       # 本文档（实施蓝图、接口冻结、任务拆分）
│   ├── DESIGN_MANIFESTO.md        # 产品设计纲领与视觉护栏
│   ├── RULES_SCOPE.md             # V1 CoC 7e 规则子集
│   └── EVAL_PLAN.md               # 三层评测与 21 条硬门禁
├── modules/
│   └── crimson_rain_manor/
│       ├── module.yaml            # 模组元信息、预制调查员、初始场景、结局条件
│       ├── scenes.yaml            # Scene Graph（允许有环、单/双向路径、可交互物）
│       ├── clues.yaml             # Clue Dependency Graph（AND/OR Predicate、核心结论）
│       ├── handouts.yaml          # 核心物证（source_facts + 排版内容 + 解锁绑定）
│       ├── npcs.yaml              # 四层 NPC（Personality / Working Knowledge / Cover / Protected Truths）
│       ├── clocks.yaml            # 明/暗 Progress Clocks 与阶段阈值
│       └── tests/                 # 模组静态可达性、无死锁、引用完整性测试
├── apps/
│   ├── server/                    # 单一 Rust 应用（vellum-server），按清晰模块分层
│   │   ├── Cargo.toml
│   │   ├── migrations/            # SQLx SQLite 迁移文件
│   │   └── src/
│   │       ├── main.rs            # 启动入口、配置加载、WAL 初始化、优雅关闭
│   │       ├── lib.rs             # 暴露 app 构造器供集成测试无端口/本地端口测试
│   │       ├── config.rs          # 环境变量与模型路由配置（mock / live）
│   │       ├── error.rs           # thiserror 统一领域错误 + HTTP 映射
│   │       ├── schemas/           # 前后端共享强类型 DTO、SSE 事件、DVP Directive Schema
│   │       ├── kernel/            # [纯确定性，零外部 I/O 依赖]
│   │       │   ├── mod.rs
│   │       │   ├── types.rs       # Investigator, SessionState, GameEvent, TurnStatus
│   │       │   ├── dice.rs        # Rng trait (OS / Seeded), d100, bonus/penalty dice
│   │       │   ├── rules.rs       # CoC 7e 检定、Luck、Push 资格、SAN、简化叙事战斗
│   │       │   ├── state.rs       # 纯函数 Reducer（GameEvent -> SessionState）、状态重建
│   │       │   ├── clues.rs       # 受限结构化 Predicate 解释器（状态谓词 vs 回合行为谓词）
│   │       │   ├── clocks.rs      # 明/暗进度钟推进与阈值触发
│   │       │   ├── access.rs      # 玩家可见投影过滤（剔除 hidden clock & raw_truth）
│   │       │   └── improvisation.rs # Off-Rail DynamicWorldPatch 9 条合法性校验
│   │       ├── modules/           # Module IR 类型、YAML Loader、三图静态校验器
│   │       ├── persistence/       # V1 SQLite-only SQLx 窄领域仓储
│   │       │   ├── db.rs          # Pool 初始化（WAL, foreign_keys=ON, busy_timeout）
│   │       │   ├── session_repo.rs
│   │       │   ├── turn_repo.rs   # 含 PendingInterrupt、Idempotency、RecoveryLease CAS
│   │       │   ├── event_repo.rs  # GameEvent 追加、Transcript、Knowledge、Beliefs、Directives
│   │       │   └── module_repo.rs # 本地 Module IR 加载与缓存
│   │       ├── stream/            # ActiveTurnBuffer + tokio::sync::Notify 无丢失 Catch-up
│   │       ├── narration/         # 上下文装配、NPC 知识/听众过滤、Bluff 标注、Narrative Directive 校验
│   │       ├── models/            # Provider Adapter Trait + MockProvider + Live HTTP Streaming Adapter
│   │       ├── runtime/           # 纯 Rust 异步编排器
│   │       │   ├── lock.rs        # SessionCommandLock（进程内每 Session 短锁 Map）
│   │       │   ├── router.rs      # InteractionRouter（Path A / B / C 路由，不确定降级为 B/C）
│   │       │   ├── orchestrator.rs# TurnOrchestrator（唯一合法 TurnStatus 状态迁移入口）
│   │       │   ├── interrupt.rs   # InterruptService（AwaitingRoll / Recovery / Push 全链路）
│   │       │   ├── recovery.rs    # 惰性 Narration Recovery（RecoveryLease CAS + 续租 + generation_id）
│   │       │   ├── ooc.rs         # 只读 OOC Assistant
│   │       │   └── debrief.rs     # 结案真相复盘派生（严格绑定已提交事件与 Module IR）
│   │       └── api/               # Axum Router、Handlers、Idempotency 中间件、SSE 端点、Utoipa OpenAPI
│   └── web/                       # React + TypeScript + Vite 前端（@vellum/web）
│       ├── package.json
│       ├── vite.config.ts
│       ├── playwright.config.ts
│       └── src/
│           ├── app/               # 路由与页面容器（Turn 0 选角、主舞台、Debrief）
│           ├── api/               # REST 客户端、强类型契约、SSE 订阅器（支持 Last-Event-ID & 重连）
│           ├── stores/            # Zustand（会话态、流式叙事态、DVP after_narration 队列、体验模式）
│           ├── hooks/             # TanStack Query hooks + SSE 生命周期同步
│           ├── components/
│           │   ├── narrative/     # NarrativeStage、TurnEntry、ActionComposer、StatusHint
│           │   ├── dice/          # InlineDice、LuckSpendPanel、PushRollFlow
│           │   ├── dynamic_views/ # DVP Component Registry、DiegeticHandout、ClockPulse、DeltaToast
│           │   ├── evidence/      # EvidenceBoardLite（线索列表、来源、玩家便签、手动连线）
│           │   ├── character/     # CharacterDossier（属性、HP/MP/SAN/Luck、技能、物品）
│           │   ├── ooc/           # OocDrawer（只读规则与状态查询）
│           │   ├── atmosphere/    # AtmosphereLayer（雨夜/烛光/雾气/低理智可关闭）
│           │   └── debrief/       # CaseDebriefView（剧透警示 + 事件溯源复盘）
│           └── styles/            # Design Tokens（双主题 + 低动效 + 视觉方向变量）
├── evals/                         # Golden Trace Replay / Agent Regression / Red Team / Playtest Rubric
├── scripts/
│   └── check-architecture-boundaries.mjs
└── .github/workflows/ci.yml
```

### 依赖方向机械守卫（`scripts/check-architecture-boundaries.mjs`）
每次本地提交与 CI PR 检查强制验证：
- `kernel/` 禁止导入 `axum`、`sqlx`、`reqwest`、`api`、`persistence`、`models`、`narration`、`stream`、`runtime`；
- `narration/` 禁止导入 `sqlx` 或 `persistence`；
- `web/src/` 禁止 `dangerouslySetInnerHTML`、`eval(`、`new Function(`；
- 根与子包 Manifest 禁止引入 `redis`、`neo4j`、`langgraph`、`celery`、`faiss`。

---

## 3. 后端核心设计（`gpt-6.2-sol high` 实施蓝图）

### 3.1 Turn 状态机与合法迁移矩阵（第 5 节）

状态集合（共 16 个显式枚举值，字段统一序列化为 `snake_case`）：
`received` | `routing` | `proposal_pending` | `awaiting_roll` | `awaiting_recovery` | `awaiting_push_description` | `awaiting_push_consequence_ack` | `awaiting_push_roll` | `committing` | `narrating` | `narration_pending` | `recovering` | `completed` | `cancelled` | `failed` | `interrupted_before_commit`

关键迁移约束（必须写成状态机单测与不允许非法迁移守卫）：
- `awaiting_roll`：仅允许通过 `POST /interrupts/{id}/resolve` 执行 `cancel`（-> `cancelled`）、`revise_action`（同一短事务取消旧 Turn 并原子创建新 Turn）或 `confirm_roll`（执行首次真实掷骰，**先落盘骰子结果**，再按结果转 `committing` 或 `awaiting_recovery`）。
- `awaiting_recovery`：已展示首次失败结果，**锁死当前 Session 的 IC Turn**，禁止 `cancel` 或新 `POST /turns`；仅允许 `accept_failure`（-> `committing`）、`spend_luck`（校验资格与余额，同事务扣 Luck 并改判 -> `committing`）、`push_roll`（校验 `push_count == 0` 与规则资格 -> `awaiting_push_description`）。
- `awaiting_push_description`：接收 `submit_push_description` -> Kernel/KP 生成并校验升级失败后果 -> `awaiting_push_consequence_ack`。
- `awaiting_push_consequence_ack`：接收 `accept_initial_failure`（放弃 Push，接受首次失败 -> `committing`）或 `confirm_push_consequence`（-> `awaiting_push_roll`）。
- `awaiting_push_roll`：内部执行态；执行第二次真实掷骰（`attempt_index = 2, push_count = 1`），落盘后**直接进入 `committing`**，不再允许取消、Luck 或二次 Push。
- 进程重启恢复标记：启动时仅将遗留在 `received / routing / proposal_pending` 的 Turn 标记为 `interrupted_before_commit`，将已提交权威状态但停在 `narrating` 的 Turn 标记为 `narration_pending`；**绝对不在启动时自动调用 LLM**。

### 3.2 并发、幂等、SSE Catch-up 与恢复租约（第 5.5 & 11 节）

1. **Session Command Lock**：
   - `DashMap<SessionId, Arc<tokio::sync::Mutex<()>>>` 或 `RwLock<HashMap<...>>`；
   - 仅包裹“读状态 + 查幂等表 + 校验活动 Turn/Interrupt + 写 SQLite 短事务”；获取结果后**立刻 `drop(guard)`**，再 `tokio::spawn` 异步执行 Proposal/Narration 任务。
2. **Idempotency-Key**：
   - 所有变更态 POST（`/sessions`、`/turns`、`/interrupts/{id}/resolve`、`/evidence/notes`）要求 `Idempotency-Key` 请求头；
   - 计算规范化 JSON 请求体 SHA-256 作为 `request_hash`；
   - 命中同 `(session_scope, command_type, idempotency_key)`：
     - `request_hash` 一致 -> 原样返回已保存的 `status_code` + `response_json`；
     - `request_hash` 不一致 -> 返回 `409 IDEMPOTENCY_KEY_REUSED`。
3. **ActiveTurnBuffer 与无丢失 SSE Catch-up**：
   - 每个活动 Turn 拥有 `Arc<ActiveTurnBuffer>`，内含 `tokio::sync::Mutex<ActiveTurnBufferState>` + `tokio::sync::Notify`；
   - 配置硬上限：`max_events = 2048`、`max_buffer_bytes = 512 * 1024`、`max_narration_bytes = 256 * 1024`，终态保留 TTL = 5 分钟（活动态永不因 TTL 清理）；
   - **订阅端强制时序**：
     ```rust
     let notified = buffer.changed.notified();
     tokio::pin!(notified);
     notified.as_mut().enable(); // 必须先注册等待，再加锁读 Buffer
     ```
   - 游标 `Last-Event-ID` 若小于 Buffer 最早保留事件序号 `min_seq - 1`，发送 `stream_reset`（携带 `current_narration_snapshot`、`generation_id`、当前已下发 directives 和状态）并更新游标，绝不静默丢事件；
   - 生产端在锁内追加事件后，先 `drop(guard)` 再调用 `changed.notify_waiters()`。
4. **Recovery Lease（SQLite CAS 唯一恢复权）**：
   - 当客户端订阅 `GET /turns/{turn_id}/events` 且 Turn 状态为 `narration_pending`（或处于过期租约的 `recovering`）时惰性触发；
   - 通过 SQLite 单事务条件更新 `INSERT ... ON CONFLICT(turn_id) DO UPDATE ... WHERE expires_at <= ?now` 与 `turns` 表状态检查获取租约，递增 `turns.narration_generation`；
   - 仅唯一获胜者启动恢复任务，首条 SSE 发送 `recovery_started` + `narration_reset { turnId, generationId, clearPartial: true }`；
   - 后台按 `TTL/3` 续租；最终写入 `transcript_entries` 并更新 Turn 为 `completed` 前，在同一事务中再次 `WHERE lease_id = ? AND generation_id = ? AND expires_at > ?now` 校验，租约丢失则立刻中止写入。

### 3.3 SQLite 核心表结构设计（SQLx Migrate，WAL + Foreign Keys + `busy_timeout = 5000ms`）

| 表名 | 核心字段与索引 | 职责 |
|---|---|---|
| `sessions` | `id PK`, `module_id`, `module_version`, `investigator_id`, `experience_mode`, `status`, `session_version`, `active_turn_id`, `created_at`, `updated_at` | 会话主表与乐观锁版本号（CAS `WHERE id = ? AND session_version = ?`） |
| `session_snapshots` | `session_id PK`, `session_version`, `state_json`, `updated_at` | 最新权威状态快照（含 Keeper 全量态，对外必须经 `access.rs` 投影） |
| `turns` | `id PK`, `session_id FK`, `turn_index`, `path_kind`, `status`, `player_input`, `narration_generation`, `committed_at`, `completed_at`, `created_at`, `updated_at` | 回合状态机主表；`(session_id, turn_index)` 唯一 |
| `game_events` | `event_id PK`, `session_id FK`, `turn_id FK`, `seq`, `event_type`, `payload_json`, `module_version`, `caused_by`, `created_at` | 追加式权威事件日志；`(session_id, seq)` 唯一 |
| `pending_interrupts` | `id PK`, `session_id FK`, `turn_id FK`, `status`, `check_payload_json`, `first_roll_json`, `push_description`, `push_consequence_json`, `resolution_json`, `created_at`, `resolved_at` | 中断与首次骰点持久化，刷新与重启可精确恢复 |
| `idempotency_records` | `(session_scope, command_type, idempotency_key) PK`, `request_hash`, `status_code`, `response_json`, `created_at` | 所有变更态 POST 命令幂等重放与冲突检测 |
| `recovery_leases` | `turn_id PK`, `lease_id`, `owner_id`, `generation_id`, `expires_at`, `updated_at` | Narration Recovery 数据库 CAS 唯一租约 |
| `transcript_entries` | `entry_id PK`, `session_id FK`, `turn_id FK`, `scene_id NOT NULL`, `speaker_entity_id`, `present_entity_ids_json`, `audible_entity_ids_json`, `visible_entity_ids_json`, `visibility_scope`, `claim_kind`, `referenced_event_ids_json`, `content`, `created_at` | 完整 IC 叙事与发言记录（历史回放唯一源；带听众/可见元数据） |
| `ooc_messages` | `id PK`, `session_id FK`, `role`, `content`, `created_at` | OOC 独立表，物理隔离，绝不混入 NPC IC 上下文 |
| `knowledge_acquisitions` | `id PK`, `session_id FK`, `entity_id`, `fact_id`, `source_event_id`, `acquisition_method`, `acquired_at_seq`, `created_at` | 仅存经 Kernel 校验的规范 `fact_id` 传播来源 |
| `npc_beliefs` | `id PK`, `session_id FK`, `entity_id`, `source_transcript_id`, `claim_summary`, `belief_status`, `confidence`, `updated_at` | NPC 主观信念（如受玩家诈唬影响），与权威事实分离 |
| `ui_directives` | `directive_id PK`, `session_id FK`, `turn_id FK`, `narration_generation`, `source`, `directive_type`, `display_timing`, `payload_json`, `displayed_at`, `seq`, `created_at` | Kernel/Narrative 指令持久化与呈现状态恢复 |
| `evidence_notes` | `note_id PK`, `session_id FK`, `note_kind` (`note`/`link`), `payload_json`, `created_at`, `updated_at` | 玩家在线索板上的手写便签与关联线，非权威事实 |
| `model_call_audit` | `id PK`, `session_id`, `turn_id`, `role`, `provider`, `model`, `proposal_raw_json`, `rejected_reason`, `latency_ms`, `ttft_ms`, `input_tokens`, `output_tokens`, `status`, `created_at` | 审计原始提案、拒绝记录与第 14.7 节延迟/成本指标 |

---

## 4. 前后端冻结契约（REST + SSE + DVP）

> 契约是 `gpt-6.2-sol high`（后端）与 `Carbon high`（前端）并行开发的共同准绳。后端通过 `Utoipa` 导出 OpenAPI JSON，前端基于相同 TypeScript 类型实现 API client 与 Mock 固件，任何字段调整须先更新本节与双端类型。

### 4.1 统一错误格式与状态码

```json
{
  "error": {
    "code": "INTERRUPT_PENDING",
    "message": "Current check awaits your decision before starting a new turn.",
    "details": { "interrupt_id": "..." }
  }
}
```

固定错误码：
- `400`: `INVALID_REQUEST`、`MISSING_IDEMPOTENCY_KEY`
- `403`: `SESSION_NOT_ENDED`、`FORBIDDEN_ACTION`
- `404`: `NOT_FOUND`
- `409`: `IDEMPOTENCY_KEY_REUSED`、`TURN_ALREADY_ACTIVE`、`INTERRUPT_PENDING`、`INTERRUPT_ALREADY_RESOLVED`、`VERSION_CONFLICT`
- `422`: `RULE_REJECTED`（非法状态迁移或行动违反物理/规则约束）

### 4.2 REST 接口清单

| Method & Path | 幂等头 | 关键职责与约束 |
|---|---|---|
| `GET /api/health` | 否 | 服务健康检查（含当前 LLM 模式 `mock`/`live`、数据库连通性） |
| `GET /api/modules/{module_id}/investigators` | 否 | 返回预制调查员列表（仅玩家可见简介、长板、动机、初始物品、合法属性） |
| `POST /api/sessions` | **必填** | 创建 Session + 调查员 + Turn 0 登记（短事务提交后再异步启动 Turn 0 开场流） |
| `GET /api/sessions/{session_id}` | 否 | 返回会话元信息、`experience_mode`、`status`、`session_version`、`active_turn_id` |
| `GET /api/sessions/{session_id}/state` | 否 | 返回**玩家可见投影**：角色卡、当前场景与可达出口、可见在场实体、已解锁线索、已释 Handout、**仅 `player_visible` 进度钟**、当前 `pending_interrupt`、历史已接受 Transcript 摘要 |
| `PATCH /api/sessions/{session_id}/mode` | 否 | 切换 `guided` / `classic` / `immersive`（仅影响后续呈现，不改历史事实） |
| `POST /api/sessions/{session_id}/turns` | **必填** | 提交玩家行动，短锁检查后立即返回 `202` + `{ turn_id, status, stream_url }`；冲突立返 `409` |
| `GET /api/sessions/{session_id}/turns/{turn_id}` | 否 | 返回 Turn 当前状态、`narration_generation`、已持久化 Transcript、待决 Interrupt、该 Turn 的 Directives 快照（`stream_reset` 与刷新恢复用） |
| `GET /api/sessions/{session_id}/turns/{turn_id}/events` | 否 | SSE 流；读取 `Last-Event-ID`；支持多订阅者、Catch-up、`stream_reset` 与惰性 Recovery Lease |
| `POST /api/sessions/{session_id}/interrupts/{interrupt_id}/resolve` | **必填** | 按当前 Interrupt 状态校验合法动作（`cancel` / `revise_action` / `confirm_roll` / `accept_failure` / `spend_luck` / `push_roll` / `submit_push_description` / `confirm_push_consequence` / `accept_initial_failure`）；绝不接受客户端骰点 |
| `POST /api/sessions/{session_id}/ooc` | 否 | 任意 Interrupt 阶段并行可用的只读 OOC 问答（不改状态、不解 Interrupt、不泄密） |
| `GET /api/sessions/{session_id}/evidence` | 否 | 线索板全量数据（权威已发现线索 + 来源事件 + 玩家便签与连线） |
| `POST /api/sessions/{session_id}/evidence/notes` | **必填** | 创建/更新/删除玩家便签与线索关联（标记 `player_note: true`） |
| `POST /api/sessions/{session_id}/directives/{directive_id}/ack` | 否 | 记录某条 `after_narration` 指令已在前端首次展示完成（`displayed_at` 持久化，刷新不重播首次动画） |
| `GET /api/sessions/{session_id}/debrief` | 否 | 仅 `session_status == ended` 可访问；返回真实经过、NPC 动机、Protected Truth 全阶段、错过线索、隐藏钟终态、玩家影响因果、多结局对比（附 `source_event_ids` 溯源与 `spoiler_warning: true`） |

### 4.3 SSE 事件契约（`ServerStreamEvent`）

每条 SSE 帧格式：
```text
id: <monotonic_seq_u64>
event: <event_type>
data: <json_payload>
```

11 种标准 `type`（camelCase JSON 字段与 `ARCHITECTURE.md` §12.3 对齐）：
1. `status_hint`: `{ type, text, turnId }`
2. `narration_delta`: `{ type, chunk, turnId, generationId }`
3. `narration_reset`: `{ type, turnId, generationId, clearPartial: true }`
4. `narration_complete`: `{ type, turnId, generationId }`
5. `ui_directive`: `{ type, directive: DynamicViewDirective }`
6. `interrupt_required`: `{ type, interrupt: PendingInterruptDTO }`
7. `state_delta`: `{ type, patch: ClientVisibleStatePatch }`
8. `stream_reset`: `{ type, turnId, snapshot: TurnStreamSnapshot }`
9. `recovery_started`: `{ type, turnId, generationId }`
10. `turn_complete`: `{ type, turnId }`
11. `turn_error`: `{ type, turnId, code, message }`

### 4.4 Dynamic View Protocol（DVP）白名单与时序合同

```ts
export interface DynamicViewDirective {
  schemaVersion: 1;
  directiveId: string;
  sessionId: string;
  turnId: string;
  narrationGeneration: number;
  source: "kernel_owned" | "narrative_owned";
  directiveType:
    | "inline_dice"
    | "atmosphere_shift"
    | "diegetic_handout"
    | "evidence_unlocked"
    | "clock_pulse"
    | "character_state_delta"
    | "narrative_card";
  displayTiming: "immediate" | "after_narration";
  displayed?: boolean;
  payload: Record<string, unknown>;
}
```

| `directiveType` | 允许 `source` | 默认 `displayTiming` | 核心 Payload 要求 |
|---|---|---|---|
| `inline_dice` | `kernel_owned` | `immediate` | `checkId`, `skill`, `difficulty`, `targetNumber`, `bonusPenaltyDice`, `attemptIndex`, `pushCount`, `rollValue?`, `resultLevel?`, `recoveryOptions?` |
| `atmosphere_shift` | `kernel_owned` / `narrative_owned` | `immediate` | `theme` (`rain_manor` / `candlelight` / `fog` / `san_strain`), `sceneTitle`, `intensity` (`subtle`/`medium`) |
| `diegetic_handout` | `kernel_owned`（预设核心）/ `narrative_owned`（临时 `canonical: false`） | `after_narration` | `handoutId`, `kind` (`letter`/`diary`/`newspaper`/`telegram`/`report`), `title`, `canonical`, `clueId?`, `sourceFacts[]`, `presentationText` |
| `evidence_unlocked` | `kernel_owned` | `after_narration` | `clueId`, `title`, `summary`, `sourceLabel` |
| `clock_pulse` | `kernel_owned` | `after_narration` | `clockId`, `name`, `previousValue`, `currentValue`, `maxValue`, `stageLabel`, `reason`（仅 `player_visible` 钟） |
| `character_state_delta` | `kernel_owned` | `immediate` 或 `after_narration` | `deltas[]`（`hp`/`mp`/`san`/`luck`/`item_added`/`item_removed`/`condition`）+ `reason` |
| `narrative_card` | `narrative_owned` | `after_narration` | 非权威氛围卡片，`canonical: false`，禁止修改数值/线索/物品 |

---

## 5. 前端架构设计（`Carbon high` 实施蓝图）

### 5.1 核心架构分层
- **服务端状态（TanStack Query）**：Session 元数据、玩家可见状态快照、历史 Turn、Evidence Board、Debrief、Investigator 列表。
- **实时与呈现状态（Zustand）**：
  - `activeTurnStream`：当前 `turnId`、`generationId`、实时拼接的 `narrationBuffer`、打字机展示进度（支持跳过纯前端视觉动画，绝不跳过未生成内容）、连接状态；
  - `pendingDirectivesQueue`：按 `(turnId, narrationGeneration)` 隔离存储 `after_narration` 指令；收到 `narration_complete` 或玩家跳过打字机后按原始 `seq` 释放渲染，并调用 `/directives/{id}/ack`；遇到 `narration_reset` 时清理旧 generation 临时文本，并将仍待展示的 `kernel_owned` 指令重绑定到新 `generationId`；
  - `displayedDirectiveIds`：基于 `directiveId` 幂等去重，防止重连/刷新重复播放首次揭晓动画；
  - `uiPreferences`：体验模式（`guided` / `classic` / `immersive`）、深浅色主题、**低理智特效开关**（默认尊重 `prefers-reduced-motion`）。

### 5.2 六大界面区域（桌面端优先，默认突出叙事）
1. **Turn 0 入口**：一键选择预制调查员（敏锐记者 / 退役警探 / 家族医生）+ 体验模式 -> 10 秒进入雨夜序幕与初始委托函。
2. **Narrative Stage（核心主栏）**：克制编辑式正文排版（单栏最佳阅读行宽 62–72ch），内嵌 NPC 对话、Inline Dice 交互卡片、经 `after_narration` 释放的 Diegetic Handout 实体物证与状态提示。
3. **Action Composer（底部粘滞输入区）**：自由文本输入 + 快捷状态指示；当存在 `PendingInterrupt` 时平滑切换为对应决策栏（掷骰前可确认/修改/撤回；失败后锁死输入并展示接受失败/消耗幸运/孤注一掷三步流程）。
4. **Character Dossier（可折叠侧抽屉/边栏）**：HP / MP / SAN / Luck 数值条（兼顾文字与图标，不单靠颜色）、核心技能、背包物品、状态标签。
5. **Evidence Board Lite（可切换面板/抽屉）**：已发现线索卡片 + 来源追溯 + 玩家自建便签 + 手动线索连线（明确区分“已证实线索”与“调查员推论笔记”）。
6. **OOC Assistant（独立轻量抽屉）**：任何 Interrupt 阶段均可打开询问规则和角色现状，视觉上明确区分“场外规则助手（不推进时间）”。
7. **Case Debrief（结案全屏阅读视图）**：仅 `ended` 态解锁，带剧透警示、事件时间轴、隐藏钟与真相阶段揭晓、玩家选择分岔对比。

### 5.3 体验模式差异矩阵（仅改呈现，不改规则）
- **Guided**：展示行动启发提示（仍始终可自由输入）、检定卡片展开规则与概率说明、适时展示当前目标摘要。
- **Classic（默认）**：平衡叙事与清晰系统交互，重要检定与物证突出呈现，普通行动零打扰。
- **Immersive**：最小化系统chrome，骰盘与提示以世界内措辞融入叙事流，但仍清晰保留不可逆风险与精确结果查看入口。

---

## 6. 分阶段里程碑与三方协作执行协议

### 6.1 角色与工具调用约定
- **主控 & 架构（Gemini 4 Neon XHigh）**：
  - 冻结架构计划、API/DVP 契约、视觉方向与首版设计样板；
  - 负责全部环境依赖安装、CI 配置、代码审查（`codex-review` 最多 2 轮）、联调、浏览器真机验收与 PR 合并发布。
- **后端实现（`gpt-6.2-sol high`）**：
  - 通过 `codex-worker --model gpt-6.2-sol --effort high` 派发成块后端里程碑任务；
  - 严格按阶段交付可编译、全绿 `cargo test` + `clippy` 代码；不擅自装系统依赖、不执行 git commit/push。
- **前端实现（`Carbon high`）**：
  - 在 Lunriv 选定视觉方向且 Neon 定好 Design Tokens 与核心样板后，通过 `agy-ask --carbon` 派发模块页面、DVP 组件族、交互态、深浅色/低理智可访问性适配与测试；
  - 严格遵守样板与契约，不擅自加多余功能或绕过 DVP 注册表。

### 6.2 阶段交付门禁（严格对齐 ARCHITECTURE.md §20.6）

| 阶段 | 负责方 | 核心交付 | 必须通过的验收门禁 |
|---|---|---|---|
| **M0：初始化与环境** | Neon | 仓库骨架、架构/规则/评测文档、CI、架构边界守卫、D 盘 Rust 工具链就绪、视觉方向定稿 | `pnpm check:arch` 绿；`cargo --version` & `pnpm --version` 就绪；首个含 CI 的初始化基线确立 |
| **M1：V0.1 + V0.2 后端底座** | `gpt-6.2-sol high`（Neon 验 diff & 跑测） | 纯函数 CoC 7e Kernel + `proptest`；16 态 TurnStateMachine；SQLite WAL Repository；PendingInterrupt；Session 短锁；Idempotency-Key；乐观并发 CAS（纯 Mock，不接真实 LLM） | 规则单测与属性测试 100%；防刷骰/防重扣/Push 上限/409/幂等重放/事务一致性测试全绿；`check:arch` 绿 |
| **M2：V0.3 API + SSE + Recovery & 前端样板** | 后端 `gpt-6.2-sol high`；前端 Neon 出核心样板 -> `Carbon high` 并行铺量 V0.5 | 后端：Axum 14 端点 + Utoipa OpenAPI + `ActiveTurnBuffer` + 多订阅者无丢失 Catch-up + `stream_reset` + `RecoveryLease` CAS + `narration_reset`。<br>前端：Turn 0、Narrative Stage、Action Composer、DVP 注册表、`after_narration` 队列、Inline Dice、Diegetic Handout、Evidence Board Lite、Character Dossier、OOC、3 模式 | 后端：慢 Mock LLM 下并发 409、多 SSE 订阅、游标过期、双客户端恢复 CAS 竞争测试全绿。<br>前端：`typecheck`/`lint`/`vitest`/`build` 全绿；Mock 联调下刷新恢复与 `after_narration` 时序正确 |
| **M3：V0.4 Module IR + NPC 认知 + LLM Adapter** | `gpt-6.2-sol high` | 三图模型 + YAML Loader + 静态无死锁校验；四层 NPC 模型 + Knowledge Provenance + Bluff (`NpcBelief`) + Protected Truth 多阶段释放；可插拔真实 LLM Streaming Adapter（兼容 Mock/Live 切换） | 线索可达性测试、跨场景串戏隔离测试、Bluff 不污染规范事实测试、Protected Truth 零泄露测试、真模型延迟指标采集 |
| **M4：V1 《绯雨庄园》完整垂直切片与验收** | 后端 `gpt-6.2-sol high` + 前端 `Carbon high` + Neon 联调验收 | 完整《绯雨庄园》（4~5 场景、3 NPC、2 明暗钟、8~12 线索、3~5 Handout、3 预制调查员、多结局）；Case Debrief；Golden Trace Replay + 14 类红队脚本 + Playwright E2E | 21 条硬门禁全部达标；Neon 在浏览器中从 Turn 0 走到结案复盘实测通过（含刷新、重连、双击 409、减动效/深浅色）；`codex-review` 审查归零 |

---

## 7. 已拍板决策记录（2026-11-26 Lunriv 确认）

1. **本机 Rust 工具链路径**：安装并配置至 **D 盘**（`RUSTUP_HOME=D:\dev\rustup`、`CARGO_HOME=D:\dev\cargo`、`CARGO_TARGET_DIR=D:\dev\cargo-target\vellum`），避免占用 C 盘剩余空间。
2. **主视觉双主题方案**：
   - **白天模式（Light）**：采用 **方向 A · 纸墨档案室**（暖棉纸底 `#F6F2E9` / 纸卡 `#FBF8F1` / 墨色正文 `#1F1B17` / 朱砂绯红重点 `#8F2424` / 深炭骰盘 `#26211C`）。
   - **夜间模式（Dark）**：采用 **方向 B · 雨夜侦探稿**（深石板蓝黑底 `#13171D` / 面板 `#1A2029` / 暖米白正文 `#E6E1D6` / 煤油灯琥珀重点 `#C78A42` / 实体物证保持泛黄高可读纸页 `#EDE4D0` 配深墨字）。
3. **本轮实施范围（方案 1）**：优先交付**完整可玩的模拟对局垂直切片**（后端 V0.1→V0.4 含纯函数 CoC 7e 内核、16 态状态机、SQLite WAL 持久化、Axum REST + SSE + `ActiveTurnBuffer` + `RecoveryLease` CAS、三图 Module IR、NPC Knowledge Provenance + Bluff + Protected Truth、Mock Provider；前端 V0.5 含 Turn 0、双主题视觉系统、Narrative Stage、Action Composer、DVP 白名单注册表、`after_narration` 队列、Inline Dice 幸运/Push、Diegetic Handout、Evidence Board Lite、Character Dossier、OOC、3 种体验模式、刷新恢复与 `narration_reset`），联调测通后再接入真实大模型与铺完完整剧情。
4. **GitHub 仓库默认分支**：使用 `main`，关联远程 `https://github.com/Lunriv/Vellum.git`。
5. **V1 CoC 7e 规则边界确认**（见 [RULES_SCOPE.md](RULES_SCOPE.md) §3）：
   - 调查检定大失败（Fumble）禁止 Push 和消耗幸运；
   - 困难/极难检定允许消耗幸运补齐至所需成功等级。
