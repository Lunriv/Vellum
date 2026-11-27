import { Compass, Eye, Flame, Lock, MessageSquare, Search } from "lucide-react";
import type { MapEdgeDTO, MapRoomDTO, SceneEntityDTO, VisibleSceneDTO } from "../../api/types";

interface DynamicWorldMapProps {
  moduleTitle?: string;
  currentScene: VisibleSceneDTO;
  mapRooms?: MapRoomDTO[];
  mapEdges?: MapEdgeDTO[];
  worldTags?: string[];
  actionLocked?: boolean;
  onExecuteAction: (actionText: string) => void;
}

interface LayoutNode {
  room: MapRoomDTO;
  x: number;
  y: number;
  w: number;
  h: number;
  cx: number;
  cy: number;
}

const ROLE_LABELS: Record<string, string> = {
  gatekeeper: "守门人",
  host: "主人",
  witness: "目击者",
  suspect: "嫌疑人",
  ally: "协助者",
  npc: "在场人物",
  object: "可查物件",
};

const TOKEN_PALETTE = ["#b6772b", "#9d3338", "#3d7a8f", "#487a4e", "#7a4e8c", "#8f5432"];

function entityTokenStyle(seed: string, name: string): { glyph: string; tone: string } {
  let hash = 0;
  for (let i = 0; i < seed.length; i += 1) {
    hash = (hash * 31 + seed.charCodeAt(i)) >>> 0;
  }
  const cleanName = name.trim();
  return {
    glyph: cleanName.slice(0, 1) || "人",
    tone: TOKEN_PALETTE[hash % TOKEN_PALETTE.length],
  };
}

function formatMoveAction(label: string | null | undefined, title: string): string {
  if (label && /^(前往|返回|回到|推门|从|绕|侧门|进入|离开|潜入|撤回)/.test(label)) {
    return label;
  }
  return `前往${label || title}`;
}

function defaultObjectAction(entity: SceneEntityDTO): string {
  return `仔细检查${entity.name}`;
}

function shortRoomTitle(title: string): string {
  return title.length > 6 ? `${title.slice(0, 6)}…` : title;
}

/**
 * Computes a clean, deterministic architectural floorplan layout for ANY module's room graph.
 * Uses BFS depth ordering from the primary hub so connected rooms sit near each other cleanly.
 */
function computeGraphLayout(
  rooms: MapRoomDTO[],
  edges: MapEdgeDTO[],
): { nodes: LayoutNode[]; width: number; height: number } {
  const width = 404;
  if (rooms.length === 0) {
    return { nodes: [], width, height: 140 };
  }

  // Build adjacency map for BFS ordering
  const adj = new Map<string, string[]>();
  for (const r of rooms) adj.set(r.sceneId, []);
  for (const e of edges) {
    adj.get(e.fromSceneId)?.push(e.toSceneId);
    adj.get(e.toSceneId)?.push(e.fromSceneId);
  }

  // Start ordering from the first room (typically initial_scene_id) or highest-degree hub
  const startId = rooms[0].sceneId;
  const ordered: MapRoomDTO[] = [];
  const visited = new Set<string>([startId]);
  const queue = [startId];
  const byId = new Map(rooms.map((r) => [r.sceneId, r]));

  while (queue.length > 0) {
    const cur = queue.shift()!;
    const room = byId.get(cur);
    if (room) ordered.push(room);
    for (const nxt of adj.get(cur) ?? []) {
      if (!visited.has(nxt) && byId.has(nxt)) {
        visited.add(nxt);
        queue.push(nxt);
      }
    }
  }
  for (const r of rooms) {
    if (!visited.has(r.sceneId)) {
      ordered.push(r);
    }
  }

  // Split into balanced rows (max 3 per row; e.g., 5 rooms -> 3 + 2, 4 rooms -> 2 + 2)
  const rows: MapRoomDTO[][] = [];
  if (ordered.length <= 3) {
    rows.push(ordered);
  } else if (ordered.length === 4) {
    rows.push(ordered.slice(0, 2), ordered.slice(2, 4));
  } else {
    for (let i = 0; i < ordered.length; i += 3) {
      rows.push(ordered.slice(i, i + 3));
    }
  }

  const rowHeight = 74;
  const rowGap = 30;
  const padTop = 20;
  const padSide = 18;
  const colGap = 16;

  const nodes: LayoutNode[] = [];
  rows.forEach((row, rIdx) => {
    const count = row.length;
    const usableW = width - padSide * 2 - colGap * (count - 1);
    const cellW = Math.min(156, Math.floor(usableW / count));
    const totalRowW = cellW * count + colGap * (count - 1);
    const startX = Math.floor((width - totalRowW) / 2);
    const y = padTop + rIdx * (rowHeight + rowGap);

    row.forEach((room, cIdx) => {
      const x = startX + cIdx * (cellW + colGap);
      nodes.push({
        room,
        x,
        y,
        w: cellW,
        h: rowHeight,
        cx: x + cellW / 2,
        cy: y + rowHeight / 2,
      });
    });
  });

  const height = padTop + rows.length * rowHeight + Math.max(0, rows.length - 1) * rowGap + 24;
  return { nodes, width, height };
}

export function DynamicWorldMap({
  moduleTitle,
  currentScene,
  mapRooms,
  mapEdges,
  worldTags = [],
  actionLocked = false,
  onExecuteAction,
}: DynamicWorldMapProps) {
  const exitByTarget = new Map(currentScene.exits.map((ex) => [ex.targetSceneId, ex]));
  const npcs = currentScene.presentEntities.filter((e) => e.kind === "npc");
  const objects = currentScene.presentEntities.filter((e) => e.kind === "object");

  // Synthesize minimal room list from currentScene + exits if mapRooms isn't supplied (e.g. legacy mock test)
  const effectiveRooms: MapRoomDTO[] =
    mapRooms && mapRooms.length > 0
      ? mapRooms
      : [
          {
            sceneId: currentScene.sceneId,
            title: currentScene.title,
            atmosphere: currentScene.atmosphere,
            visited: true,
            isCurrent: true,
            accessibleFromCurrent: false,
            exitLabelFromCurrent: null,
            knownNpcs: npcs.map((n) => ({ entityId: n.entityId, name: n.name })),
          },
          ...currentScene.exits.map((ex) => ({
            sceneId: ex.targetSceneId,
            title: ex.label.replace(/^(前往|返回|回到)/, "") || ex.targetSceneId,
            atmosphere: currentScene.atmosphere,
            visited: Boolean(currentScene.visitedSceneIds?.includes(ex.targetSceneId)),
            isCurrent: false,
            accessibleFromCurrent: ex.accessible,
            exitLabelFromCurrent: ex.label,
            knownNpcs: [],
          })),
        ];

  const effectiveEdges: MapEdgeDTO[] =
    mapEdges && mapEdges.length > 0
      ? mapEdges
      : currentScene.exits.map((ex) => ({
          fromSceneId: currentScene.sceneId,
          toSceneId: ex.targetSceneId,
          bidirectional: true,
          locked: !ex.accessible,
        }));

  const { nodes, width, height } = computeGraphLayout(effectiveRooms, effectiveEdges);
  const nodeById = new Map(nodes.map((n) => [n.room.sceneId, n]));
  const visibleTags = worldTags.filter((t) => !t.startsWith("patch:")).slice(-4);

  return (
    <section
      aria-label="动态世界沙盘"
      className="vellum-wood-panel overflow-hidden rounded-[var(--vellum-radius-lg)]"
    >
      <div className="flex items-center justify-between gap-2 border-b border-[var(--vellum-wood-border)]/70 px-3.5 py-2">
        <div className="inline-flex min-w-0 items-center gap-1.5 text-xs font-semibold tracking-wide text-[var(--vellum-brass)]">
          <Compass className="h-3.5 w-3.5 shrink-0" />
          <span className="truncate">{moduleTitle ? `${moduleTitle} · 沙盘` : "动态世界沙盘"}</span>
        </div>
        <span className="shrink-0 rounded bg-[var(--vellum-elevated)] px-2 py-0.5 text-[11px] font-medium text-[var(--vellum-ink)]">
          {currentScene.title}
        </span>
      </div>

      <div className="p-3">
        <div className="vellum-parchment-map relative overflow-hidden rounded-[var(--vellum-radius-md)] p-2">
          <svg
            viewBox={`0 0 ${width} ${height}`}
            className="h-auto w-full select-none"
            role="img"
            aria-label={`${moduleTitle ?? "世界"}区域关系图，当前位于${currentScene.title}`}
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

            <rect
              x="5"
              y="5"
              width={width - 10}
              height={height - 10}
              rx="6"
              fill="none"
              stroke="#8c714c"
              strokeWidth="1.2"
              strokeDasharray="4 3"
              opacity="0.7"
            />

            {/* Dynamic corridors between connected rooms */}
            {effectiveEdges.map((edge, idx) => {
              const a = nodeById.get(edge.fromSceneId);
              const b = nodeById.get(edge.toSceneId);
              if (!a || !b) return null;
              const dashed = edge.locked || !edge.bidirectional;
              return (
                <line
                  key={`${edge.fromSceneId}-${edge.toSceneId}-${idx}`}
                  x1={a.cx}
                  y1={a.cy}
                  x2={b.cx}
                  y2={b.cy}
                  stroke={edge.locked ? "#8f3228" : "#6f5737"}
                  strokeWidth={dashed ? "1.6" : "2.4"}
                  strokeDasharray={dashed ? "4 3" : undefined}
                  opacity={dashed ? 0.65 : 0.82}
                />
              );
            })}

            {/* Dynamic interactive room nodes */}
            {nodes.map(({ room, x, y, w, h, cx }) => {
              const exitInfo = exitByTarget.get(room.sceneId);
              const isCurrent = room.isCurrent || currentScene.sceneId === room.sceneId;
              const visited = room.visited || isCurrent;
              const accessible = room.accessibleFromCurrent || Boolean(exitInfo?.accessible);
              const isBlockedExit = Boolean(exitInfo && !exitInfo.accessible);
              const moveAction = formatMoveAction(
                room.exitLabelFromCurrent ?? exitInfo?.label,
                room.title,
              );
              const clickable = !isCurrent && accessible && !actionLocked;
              const knownNpcs = isCurrent
                ? npcs.map((n) => ({ entityId: n.entityId, name: n.name }))
                : room.knownNpcs;

              return (
                <g
                  key={room.sceneId}
                  role={clickable ? "button" : undefined}
                  tabIndex={clickable ? 0 : undefined}
                  aria-label={
                    isCurrent
                      ? `当前所在房间：${room.title}`
                      : clickable
                        ? `地图移动：${moveAction}`
                        : `${room.title}（${isBlockedExit ? "暂未解锁" : "未相邻"}）`
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
                  <rect
                    x={x}
                    y={y}
                    width={w}
                    height={h}
                    rx="6"
                    fill={isCurrent ? "#f7ebd0" : visited ? "#ebd9b7" : "#dcc7a0"}
                    stroke={isCurrent ? "#9f2a23" : clickable ? "#8a5a18" : "#8c714c"}
                    strokeWidth={isCurrent ? "2.4" : clickable ? "1.8" : "1.2"}
                  />

                  {!visited ? (
                    <rect
                      x={x + 1}
                      y={y + 1}
                      width={w - 2}
                      height={h - 2}
                      rx="5"
                      fill="url(#vellum-fog-hatch)"
                    />
                  ) : null}

                  {isCurrent ? (
                    <rect
                      x={x - 2}
                      y={y - 2}
                      width={w + 4}
                      height={h + 4}
                      rx="8"
                      fill="none"
                      stroke="#d89d3e"
                      strokeWidth="1.4"
                      strokeOpacity="0.85"
                    />
                  ) : null}

                  <text x={x + 8} y={y + 18} fontSize="11" fontWeight="700" fill="#2b1e12">
                    {shortRoomTitle(room.title)}
                  </text>

                  <text
                    x={x + w - 7}
                    y={y + 17}
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

                  {isCurrent ? (
                    <g transform={`translate(${x + 24}, ${y + 49})`}>
                      <circle
                        r="13"
                        fill="none"
                        stroke="#c6372c"
                        strokeWidth="1.6"
                        className="vellum-beacon-ring"
                      />
                      <circle r="10.5" fill="#9f2a23" stroke="#f9eed6" strokeWidth="1.8" />
                      <text y="3.8" textAnchor="middle" fontSize="10" fontWeight="700" fill="#fdf8ee">
                        你
                      </text>
                    </g>
                  ) : null}

                  {knownNpcs.slice(0, 2).map((npc, idx) => {
                    const { glyph, tone } = entityTokenStyle(npc.entityId, npc.name);
                    const tx = isCurrent ? x + 52 + idx * 24 : x + 24 + idx * 24;
                    const ty = y + 49;
                    return (
                      <g key={npc.entityId} transform={`translate(${tx}, ${ty})`}>
                        <circle r="9.5" fill={tone} stroke="#f7ebd0" strokeWidth="1.5" />
                        <text y="3.5" textAnchor="middle" fontSize="9" fontWeight="700" fill="#fff8ec">
                          {glyph}
                        </text>
                      </g>
                    );
                  })}

                  {!visited && knownNpcs.length === 0 ? (
                    <text
                      x={cx}
                      y={y + 52}
                      textAnchor="middle"
                      fontSize="9.5"
                      fill="#5f4a30"
                      opacity="0.85"
                    >
                      {isBlockedExit ? "路径受阻" : "迷雾笼罩"}
                    </text>
                  ) : null}
                </g>
              );
            })}
          </svg>

          <div className="mt-1.5 flex flex-wrap items-center justify-between gap-2 px-1 text-[10px] text-[#574229]">
            <span>点击高亮相邻区域可直接移动</span>
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

        {/* Active Off-Rail World Alterations */}
        {visibleTags.length > 0 ? (
          <div className="mt-2.5 flex flex-wrap items-center gap-1.5 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-brass)]/40 bg-[var(--vellum-brass-soft)] px-2.5 py-1.5 text-[11px]">
            <Flame className="h-3 w-3 shrink-0 text-[var(--vellum-brass)]" />
            <span className="font-semibold text-[var(--vellum-ink)]">世界现场改动：</span>
            {visibleTags.map((tag) => (
              <span
                key={tag}
                className="vellum-mono rounded bg-[var(--vellum-surface)] px-1.5 py-0.2 text-[10px] text-[var(--vellum-ink)]"
              >
                {tag}
              </span>
            ))}
          </div>
        ) : null}

        {/* Present Interactive Stage Entities */}
        {currentScene.presentEntities.length > 0 ? (
          <div className="mt-3 space-y-2">
            <div className="flex items-center justify-between text-[11px] font-semibold text-[var(--vellum-muted)]">
              <span>当前场景互动棋子（点击直接行动）</span>
              <span>{currentScene.presentEntities.length} 处目标</span>
            </div>

            {npcs.length > 0 ? (
              <div className="space-y-1.5">
                {npcs.map((npc) => {
                  const { glyph, tone } = entityTokenStyle(npc.entityId, npc.name);
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
                              onClick={() => onExecuteAction(`上前与${npc.name}交谈，打探现场情况`)}
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
