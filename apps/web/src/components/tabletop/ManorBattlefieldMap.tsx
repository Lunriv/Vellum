import { Compass, Eye, Lock, MessageSquare, Search } from "lucide-react";
import type { MapRoomDTO, SceneEntityDTO, VisibleSceneDTO } from "../../api/types";

interface ManorBattlefieldMapProps {
  currentScene: VisibleSceneDTO;
  mapRooms?: MapRoomDTO[];
  actionLocked?: boolean;
  onExecuteAction: (actionText: string) => void;
}

interface RoomGeometry {
  sceneId: string;
  shortTitle: string;
  x: number;
  y: number;
  w: number;
  h: number;
  cx: number;
  cy: number;
}

const MANOR_GEOMETRY: RoomGeometry[] = [
  { sceneId: "study_room", shortTitle: "书房", x: 18, y: 20, w: 112, h: 74, cx: 74, cy: 57 },
  { sceneId: "manor_foyer", shortTitle: "门厅外廊", x: 146, y: 20, w: 112, h: 74, cx: 202, cy: 57 },
  { sceneId: "maids_parlor", shortTitle: "女仆偏厅", x: 274, y: 20, w: 112, h: 74, cx: 330, cy: 57 },
  {
    sceneId: "rain_courtyard",
    shortTitle: "暴雨庭院",
    x: 42,
    y: 124,
    w: 142,
    h: 74,
    cx: 113,
    cy: 161,
  },
  {
    sceneId: "cellar_entrance",
    shortTitle: "酒窖入口",
    x: 220,
    y: 124,
    w: 142,
    h: 74,
    cx: 291,
    cy: 161,
  },
];

const MANOR_CORRIDORS: Array<{ from: [number, number]; to: [number, number]; dashed?: boolean }> = [
  { from: [130, 57], to: [146, 57] }, // study <-> foyer
  { from: [258, 57], to: [274, 57] }, // foyer <-> parlor
  { from: [74, 94], to: [88, 124] }, // study <-> courtyard
  { from: [184, 94], to: [146, 124] }, // foyer <-> courtyard
  { from: [224, 94], to: [262, 124], dashed: true }, // foyer <-> cellar
  { from: [184, 161], to: [220, 161], dashed: true }, // courtyard -> cellar back window/door
];

const ROLE_LABELS: Record<string, string> = {
  gatekeeper: "守门人",
  host: "庄园主人",
  witness: "目击者",
  suspect: "相关人物",
  ally: "协助者",
  npc: "在场人物",
  object: "可查物件",
};

function npcMonogram(entityId: string, name: string): { glyph: string; tone: string } {
  if (entityId.includes("arthur") || name.includes("Arthur")) {
    return { glyph: "管", tone: "#b6772b" };
  }
  if (entityId.includes("evelyn") || name.includes("Evelyn")) {
    return { glyph: "伊", tone: "#9d3338" };
  }
  if (entityId.includes("mary") || name.includes("Mary")) {
    return { glyph: "玛", tone: "#3d7a8f" };
  }
  return { glyph: name.slice(0, 1) || "人", tone: "#8f4b32" };
}

function formatMoveAction(label: string | null | undefined, title: string): string {
  if (label && /^(前往|返回|回到|推门|从|绕|侧门|进入|离开)/.test(label)) {
    return label;
  }
  return `前往${label || title}`;
}

function defaultObjectAction(entity: SceneEntityDTO): string {
  if (entity.entityId === "study_desk") return "仔细检查书桌与锁住的抽屉";
  if (entity.entityId === "cellar_door") return "查看酒窖铁门与门锁";
  if (entity.entityId === "service_tray") return "检查银托盘背面的刻纹";
  if (entity.entityId === "greenhouse") return "查看荒废温室外的泥痕与脚印";
  return `仔细检查${entity.name}`;
}

export function ManorBattlefieldMap({
  currentScene,
  mapRooms,
  actionLocked = false,
  onExecuteAction,
}: ManorBattlefieldMapProps) {
  const roomMap = new Map<string, MapRoomDTO>();
  if (mapRooms) {
    for (const r of mapRooms) {
      roomMap.set(r.sceneId, r);
    }
  }

  const isManorScenario =
    MANOR_GEOMETRY.some((g) => g.sceneId === currentScene.sceneId) ||
    Boolean(mapRooms && mapRooms.some((r) => r.sceneId === "manor_foyer"));

  const exitByTarget = new Map(currentScene.exits.map((ex) => [ex.targetSceneId, ex]));
  const visitedSet = new Set(currentScene.visitedSceneIds ?? [currentScene.sceneId]);
  const npcs = currentScene.presentEntities.filter((e) => e.kind === "npc");
  const objects = currentScene.presentEntities.filter((e) => e.kind === "object");

  return (
    <section
      aria-label="庄园战术沙盘"
      className="vellum-wood-panel overflow-hidden rounded-[var(--vellum-radius-lg)]"
    >
      <div className="flex items-center justify-between border-b border-[var(--vellum-wood-border)]/70 px-3.5 py-2">
        <div className="inline-flex items-center gap-1.5 text-xs font-semibold tracking-wide text-[var(--vellum-brass)]">
          <Compass className="h-3.5 w-3.5" />
          <span>绯雨庄园 · 战局沙盘</span>
        </div>
        <span className="rounded bg-[var(--vellum-elevated)] px-2 py-0.5 text-[11px] font-medium text-[var(--vellum-ink)]">
          当前：{currentScene.title}
        </span>
      </div>

      <div className="p-3">
        {isManorScenario ? (
          <div className="vellum-parchment-map relative overflow-hidden rounded-[var(--vellum-radius-md)] p-2">
            <svg
              viewBox="0 0 404 218"
              className="h-auto w-full select-none"
              role="img"
              aria-label={`绯雨庄园平面图，当前位于${currentScene.title}`}
            >
              <defs>
                <pattern
                  id="vellum-fog-hatch"
                  width="7"
                  height="7"
                  patternTransform="rotate(45)"
                  patternUnits="userSpaceOnUse"
                >
                  <line
                    x1="0"
                    y1="0"
                    x2="0"
                    y2="7"
                    stroke="#6b5638"
                    strokeWidth="1.2"
                    strokeOpacity="0.28"
                  />
                </pattern>
              </defs>

              {/* Outer hand-drawn parchment border */}
              <rect
                x="5"
                y="5"
                width="394"
                height="208"
                rx="6"
                fill="none"
                stroke="#8c714c"
                strokeWidth="1.2"
                strokeDasharray="4 3"
                opacity="0.7"
              />

              {/* Subtle architectural compass mark */}
              <g transform="translate(384, 198)" opacity="0.55">
                <circle r="8" fill="none" stroke="#6f5737" strokeWidth="0.9" />
                <path d="M0 -6 L2 0 L0 6 L-2 0 Z" fill="#6f5737" />
                <text
                  x="0"
                  y="-10"
                  textAnchor="middle"
                  fontSize="7"
                  fontWeight="700"
                  fill="#5a452a"
                >
                  N
                </text>
              </g>

              {/* Corridors */}
              {MANOR_CORRIDORS.map((c, idx) => (
                <line
                  key={idx}
                  x1={c.from[0]}
                  y1={c.from[1]}
                  x2={c.to[0]}
                  y2={c.to[1]}
                  stroke="#6f5737"
                  strokeWidth={c.dashed ? "1.6" : "2.4"}
                  strokeDasharray={c.dashed ? "4 3" : undefined}
                  opacity={c.dashed ? 0.65 : 0.85}
                />
              ))}

              {/* Interactive Rooms */}
              {MANOR_GEOMETRY.map((geom) => {
                const roomMeta = roomMap.get(geom.sceneId);
                const exitInfo = exitByTarget.get(geom.sceneId);
                const isCurrent = roomMeta ? roomMeta.isCurrent : currentScene.sceneId === geom.sceneId;
                const visited = roomMeta ? roomMeta.visited : isCurrent || visitedSet.has(geom.sceneId);
                const accessible = roomMeta
                  ? roomMeta.accessibleFromCurrent
                  : Boolean(exitInfo?.accessible);
                const isBlockedExit = Boolean(exitInfo && !exitInfo.accessible);
                const moveAction = formatMoveAction(
                  roomMeta?.exitLabelFromCurrent ?? exitInfo?.label,
                  roomMeta?.title ?? geom.shortTitle,
                );
                const clickable = !isCurrent && accessible && !actionLocked;

                const knownNpcs = isCurrent
                  ? npcs.map((n) => ({ entityId: n.entityId, name: n.name }))
                  : (roomMeta?.knownNpcs ?? []);

                return (
                  <g
                    key={geom.sceneId}
                    role={clickable ? "button" : undefined}
                    tabIndex={clickable ? 0 : undefined}
                    aria-label={
                      isCurrent
                        ? `当前所在房间：${roomMeta?.title ?? geom.shortTitle}`
                        : clickable
                          ? `地图移动：${moveAction}`
                          : `${roomMeta?.title ?? geom.shortTitle}（${isBlockedExit ? "暂未解锁" : "未相邻"}）`
                    }
                    onClick={() => {
                      if (clickable) onExecuteAction(moveAction);
                    }}
                    onKeyDown={(e) => {
                      if (clickable && (e.key === "Enter" || e.key === " ")) {
                        e.preventDefault();
                        onExecuteAction(moveAction);
                      }
                    }}
                    className={clickable ? "cursor-pointer" : "cursor-default"}
                  >
                    {/* Room floor rectangle */}
                    <rect
                      x={geom.x}
                      y={geom.y}
                      width={geom.w}
                      height={geom.h}
                      rx="6"
                      fill={
                        isCurrent
                          ? "#f7ebd0"
                          : visited
                            ? "#ebd9b7"
                            : "#dcc7a0"
                      }
                      stroke={
                        isCurrent
                          ? "#9f2a23"
                          : clickable
                            ? "#8a5a18"
                            : "#8c714c"
                      }
                      strokeWidth={isCurrent ? "2.4" : clickable ? "1.8" : "1.2"}
                    />

                    {/* Fog of war overlay for unvisited rooms */}
                    {!visited ? (
                      <rect
                        x={geom.x + 1}
                        y={geom.y + 1}
                        width={geom.w - 2}
                        height={geom.h - 2}
                        rx="5"
                        fill="url(#vellum-fog-hatch)"
                      />
                    ) : null}

                    {/* Current room warm lantern highlight */}
                    {isCurrent ? (
                      <rect
                        x={geom.x - 2}
                        y={geom.y - 2}
                        width={geom.w + 4}
                        height={geom.h + 4}
                        rx="8"
                        fill="none"
                        stroke="#d89d3e"
                        strokeWidth="1.4"
                        strokeOpacity="0.85"
                      />
                    ) : null}

                    {/* Room title */}
                    <text
                      x={geom.x + 9}
                      y={geom.y + 18}
                      fontSize="11.5"
                      fontWeight="700"
                      fill="#2b1e12"
                    >
                      {geom.shortTitle}
                    </text>

                    {/* Status corner label */}
                    <text
                      x={geom.x + geom.w - 8}
                      y={geom.y + 17}
                      textAnchor="end"
                      fontSize="8.5"
                      fontWeight="600"
                      fill={isCurrent ? "#9f2a23" : clickable ? "#7a4c10" : "#6f5a3e"}
                    >
                      {isCurrent
                        ? "当前"
                        : clickable
                          ? "前往 →"
                          : isBlockedExit
                            ? "上锁"
                            : visited
                              ? "已探"
                              : "未探"}
                    </text>

                    {/* Player token in current room */}
                    {isCurrent ? (
                      <g transform={`translate(${geom.x + 26}, ${geom.y + 49})`}>
                        <circle
                          r="13"
                          fill="none"
                          stroke="#c6372c"
                          strokeWidth="1.6"
                          className="vellum-beacon-ring"
                        />
                        <circle
                          r="10.5"
                          fill="#9f2a23"
                          stroke="#f9eed6"
                          strokeWidth="1.8"
                        />
                        <text
                          y="3.8"
                          textAnchor="middle"
                          fontSize="10"
                          fontWeight="700"
                          fill="#fdf8ee"
                        >
                          你
                        </text>
                      </g>
                    ) : null}

                    {/* Known NPC tokens (ONLY rendered when room is visited!) */}
                    {knownNpcs.slice(0, 2).map((npc, idx) => {
                      const { glyph, tone } = npcMonogram(npc.entityId, npc.name);
                      const tx = isCurrent ? geom.x + 56 + idx * 26 : geom.x + 26 + idx * 26;
                      const ty = geom.y + 49;
                      return (
                        <g key={npc.entityId} transform={`translate(${tx}, ${ty})`}>
                          <circle r="10" fill={tone} stroke="#f7ebd0" strokeWidth="1.6" />
                          <text
                            y="3.5"
                            textAnchor="middle"
                            fontSize="9.5"
                            fontWeight="700"
                            fill="#fff8ec"
                          >
                            {glyph}
                          </text>
                        </g>
                      );
                    })}

                    {/* Fog hint text for unvisited accessible room */}
                    {!visited && knownNpcs.length === 0 ? (
                      <text
                        x={geom.cx}
                        y={geom.y + 52}
                        textAnchor="middle"
                        fontSize="9.5"
                        fill="#5f4a30"
                        opacity="0.85"
                      >
                        {isBlockedExit ? "铁锁紧闭" : "迷雾笼罩"}
                      </text>
                    ) : null}
                  </g>
                );
              })}
            </svg>

            <div className="mt-1.5 flex flex-wrap items-center justify-between gap-2 px-1 text-[10px] text-[#574229]">
              <span>点击高亮相邻房间可直接移动</span>
              <span className="inline-flex items-center gap-2">
                <span className="inline-flex items-center gap-1">
                  <span className="inline-block h-2.5 w-2.5 rounded-full bg-[#9f2a23]" />
                  <span>调查员</span>
                </span>
                <span className="inline-flex items-center gap-1">
                  <span className="inline-block h-2.5 w-2.5 rounded-full bg-[#b6772b]" />
                  <span>已见人物</span>
                </span>
              </span>
            </div>
          </div>
        ) : (
          <div className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-elevated)]/60 p-3 text-xs">
            <p className="text-[var(--vellum-muted)]">{currentScene.description}</p>
          </div>
        )}

        {/* Present Interactive Stage Entities: 1-click NPC dialogue & Object inspection */}
        {currentScene.presentEntities.length > 0 ? (
          <div className="mt-3 space-y-2">
            <div className="flex items-center justify-between text-[11px] font-semibold text-[var(--vellum-muted)]">
              <span>当前场景互动棋子（点击直接行动）</span>
              <span>{currentScene.presentEntities.length} 处目标</span>
            </div>

            {npcs.length > 0 ? (
              <div className="space-y-1.5">
                {npcs.map((npc) => {
                  const { glyph, tone } = npcMonogram(npc.entityId, npc.name);
                  const roleLabel = ROLE_LABELS[npc.role] ?? npc.role;
                  return (
                    <div
                      key={npc.entityId}
                      className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-elevated)]/85 p-2.5"
                    >
                      <div className="flex items-start gap-2.5">
                        <span
                          className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-[var(--vellum-brass)]/70 text-xs font-bold text-[#fff8ec] shadow-xs"
                          style={{ backgroundColor: tone }}
                          aria-hidden="true"
                        >
                          {glyph}
                        </span>
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center justify-between gap-2">
                            <span className="truncate text-xs font-semibold text-[var(--vellum-ink)]">
                              {npc.name}
                            </span>
                            <span className="shrink-0 rounded bg-[var(--vellum-surface)] px-1.5 py-0.5 text-[10px] text-[var(--vellum-muted)]">
                              {roleLabel}
                            </span>
                          </div>
                          {npc.summary ? (
                            <p className="mt-0.5 text-[11px] leading-4 text-[var(--vellum-muted)]">
                              {npc.summary}
                            </p>
                          ) : null}
                          <div className="mt-2 flex flex-wrap gap-1.5">
                            <button
                              type="button"
                              disabled={actionLocked}
                              onClick={() => onExecuteAction(`上前与${npc.name}交谈，询问庄园近况`)}
                              className="inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border-strong)] bg-[var(--vellum-surface)] px-2 py-1 text-[11px] font-medium text-[var(--vellum-ink)] transition-colors hover:border-[var(--vellum-accent)] hover:text-[var(--vellum-accent)] disabled:cursor-not-allowed disabled:opacity-50"
                            >
                              <MessageSquare className="h-3 w-3" />
                              <span>开口交谈</span>
                            </button>
                            <button
                              type="button"
                              disabled={actionLocked}
                              onClick={() =>
                                onExecuteAction(`[心理学] 察言观色，观察${npc.name}的微表情与破绽`)
                              }
                              className="inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-spk-skill)]/45 bg-[var(--vellum-spk-skill-bg)] px-2 py-1 text-[11px] font-medium text-[var(--vellum-spk-skill)] transition-colors hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-50"
                            >
                              <Eye className="h-3 w-3" />
                              <span>察言观色</span>
                            </button>
                          </div>
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            ) : null}

            {objects.length > 0 ? (
              <div className="flex flex-wrap gap-1.5">
                {objects.map((obj) => (
                  <button
                    key={obj.entityId}
                    type="button"
                    disabled={actionLocked}
                    title={obj.summary}
                    onClick={() => onExecuteAction(defaultObjectAction(obj))}
                    className="inline-flex items-center gap-1.5 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-wood-border)]/70 bg-[var(--vellum-surface)] px-2.5 py-1.5 text-xs font-medium text-[var(--vellum-ink)] transition-colors hover:border-[var(--vellum-accent)] hover:bg-[var(--vellum-accent-soft)] disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    <Search className="h-3 w-3 text-[var(--vellum-brass)]" />
                    <span>{obj.name}</span>
                    <span className="text-[10px] text-[var(--vellum-muted)]">
                      {ROLE_LABELS[obj.role] ?? "可查物件"}
                    </span>
                  </button>
                ))}
              </div>
            ) : null}

            {/* Locked exits notice if any */}
            {currentScene.exits.some((ex) => !ex.accessible) ? (
              <div className="flex flex-wrap gap-1.5 pt-0.5">
                {currentScene.exits
                  .filter((ex) => !ex.accessible)
                  .map((ex) => (
                    <span
                      key={ex.targetSceneId}
                      className="inline-flex items-center gap-1 rounded bg-[var(--vellum-elevated)] px-2 py-0.5 text-[10px] text-[var(--vellum-muted)]"
                    >
                      <Lock className="h-2.5 w-2.5" />
                      <span>
                        {ex.label}：{ex.blockedReason ?? "暂不可通行"}
                      </span>
                    </span>
                  ))}
              </div>
            ) : null}
          </div>
        ) : null}
      </div>
    </section>
  );
}
