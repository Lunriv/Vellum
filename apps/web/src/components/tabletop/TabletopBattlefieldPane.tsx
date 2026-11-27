import { useEffect, useState, type ReactNode } from "react";
import {
  ArrowUpRight,
  Backpack,
  ChevronDown,
  ChevronUp,
  Clock,
  Eye,
  Heart,
  ShieldAlert,
  Sparkles,
  X,
  Zap,
} from "lucide-react";
import type {
  InvestigatorStateDTO,
  MapRoomDTO,
  PendingInterruptDTO,
  ResolveInterruptRequest,
  RollAttemptDTO,
  VisibleClockDTO,
  VisibleSceneDTO,
} from "../../api/types";
import { DiceTrayWidget } from "./DiceTrayWidget";
import { ManorBattlefieldMap } from "./ManorBattlefieldMap";

interface TabletopBattlefieldPaneProps {
  investigator: InvestigatorStateDTO;
  currentScene: VisibleSceneDTO;
  mapRooms?: MapRoomDTO[];
  visibleClocks: VisibleClockDTO[];
  pendingInterrupt: PendingInterruptDTO | null;
  lastRoll?: RollAttemptDTO | null;
  isResolvingInterrupt?: boolean;
  mobileDrawerOpen: boolean;
  actionLocked?: boolean;
  onCloseMobileDrawer: () => void;
  onExecuteAction: (actionText: string) => void;
  onQuickFillAction: (actionText: string) => void;
  onResolveInterrupt: (payload: ResolveInterruptRequest) => void;
}

const CHARACTERISTIC_LABELS: Record<string, string> = {
  STR: "力量",
  CON: "体质",
  SIZ: "体型",
  DEX: "敏捷",
  APP: "外貌",
  INT: "智力",
  POW: "意志",
  EDU: "教育",
};

const SKILL_LABELS: Record<string, string> = {
  spot_hidden: "侦查",
  listen: "聆听",
  library_use: "图书馆使用",
  psychology: "心理学",
  persuade: "说服",
  fast_talk: "话术",
  charm: "取悦",
  intimidate: "恐吓",
  medicine: "医学",
  first_aid: "急救",
  occult: "神秘学",
  history: "历史",
  law: "法律",
  stealth: "潜行",
  locksmith: "锁匠",
  sleight_of_hand: "妙手",
  track: "追踪",
  accounting: "会计",
  appraise: "估价",
  archaeology: "考古学",
  science: "科学",
  photography: "摄影",
  fighting: "格斗",
  fighting_brawl: "斗殴",
  dodge: "闪避",
  firearms_handgun: "手枪",
  credit_rating: "信用评级",
  cthulhu_mythos: "克苏鲁神话",
};

function formatExitAction(label: string): string {
  return /^(前往|返回|回到|推门|从|绕|侧门|进入|离开)/.test(label) ? label : `前往${label}`;
}

function formatKeyLabel(key: string, map: Record<string, string>): string {
  const exact = map[key] ?? map[key.toUpperCase()] ?? map[key.toLowerCase()];
  if (exact) return exact;
  return key.replace(/_/g, " ");
}

function CompactStatBar({
  label,
  current,
  max,
  icon,
  tone,
}: {
  label: string;
  current: number;
  max: number;
  icon: ReactNode;
  tone: "danger" | "accent" | "warning" | "success";
}) {
  const pct = Math.max(0, Math.min(100, Math.round((current / Math.max(1, max)) * 100)));
  const fillClass =
    tone === "danger"
      ? "bg-[var(--vellum-danger)]"
      : tone === "warning"
        ? "bg-[var(--vellum-warning)]"
        : tone === "success"
          ? "bg-[var(--vellum-success)]"
          : "bg-[var(--vellum-accent)]";

  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between text-[11px]">
        <span className="inline-flex items-center gap-1 font-medium text-[var(--vellum-ink)]">
          {icon}
          <span>{label}</span>
        </span>
        <span className="vellum-mono font-semibold text-[var(--vellum-ink)]">
          {current}/{max}
        </span>
      </div>
      <div
        role="progressbar"
        aria-label={label}
        aria-valuenow={current}
        aria-valuemin={0}
        aria-valuemax={max}
        className="h-1.5 w-full overflow-hidden rounded-full bg-[var(--vellum-border)]"
      >
        <div className={`h-full transition-all duration-200 ${fillClass}`} style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}

function TabletopBody({
  investigator,
  currentScene,
  mapRooms,
  visibleClocks,
  pendingInterrupt,
  lastRoll,
  isResolvingInterrupt,
  actionLocked = false,
  onExecuteAction,
  onQuickFillAction,
  onResolveInterrupt,
}: Omit<TabletopBattlefieldPaneProps, "mobileDrawerOpen" | "onCloseMobileDrawer">) {
  const [dossierExpanded, setDossierExpanded] = useState(false);

  const sortedSkills = Object.entries(investigator.skills).sort((a, b) => b[1] - a[1]);
  const characteristics = Object.entries(investigator.characteristics);
  const monogram = investigator.name.slice(0, 1) || "探";

  return (
    <div className="space-y-3.5 p-3.5 text-xs">
      {/* 1. Investigator Tabletop Crest + 4 Resource Bars + Visible Crisis Clock */}
      <section
        aria-label="调查员状态"
        className="vellum-wood-panel rounded-[var(--vellum-radius-lg)] p-3.5"
      >
        <div className="flex items-center gap-3">
          <div
            className="vellum-prose flex h-10 w-10 shrink-0 items-center justify-center rounded-full border-2 border-[var(--vellum-brass)] bg-[var(--vellum-accent)] text-base font-bold text-[var(--vellum-accent-ink)] shadow-xs"
            aria-hidden="true"
          >
            {monogram}
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex items-baseline justify-between gap-2">
              <h2 className="vellum-prose truncate text-base font-bold text-[var(--vellum-ink)]">
                {investigator.name}
              </h2>
              <span className="shrink-0 text-[11px] font-medium text-[var(--vellum-brass)]">
                {investigator.occupation}
              </span>
            </div>
            {investigator.conditions.length > 0 ? (
              <div className="mt-1 flex flex-wrap gap-1">
                {investigator.conditions.map((cond) => (
                  <span
                    key={cond}
                    className="inline-flex items-center gap-0.5 rounded bg-[var(--vellum-warning-soft)] px-1.5 py-0.2 text-[10px] font-medium text-[var(--vellum-warning)]"
                  >
                    <ShieldAlert className="h-2.5 w-2.5" />
                    <span>{cond}</span>
                  </span>
                ))}
              </div>
            ) : (
              <p className="mt-0.5 text-[11px] text-[var(--vellum-muted)]">身心状态平稳</p>
            )}
          </div>
        </div>

        <div className="mt-3 grid grid-cols-2 gap-x-3 gap-y-2">
          <CompactStatBar
            label="生命 HP"
            current={investigator.hp}
            max={investigator.hpMax}
            icon={<Heart className="h-3 w-3 text-[var(--vellum-danger)]" />}
            tone="danger"
          />
          <CompactStatBar
            label="理智 SAN"
            current={investigator.san}
            max={investigator.sanMax}
            icon={<Eye className="h-3 w-3 text-[var(--vellum-warning)]" />}
            tone="warning"
          />
          <CompactStatBar
            label="幸运 Luck"
            current={investigator.luck}
            max={investigator.luckMax}
            icon={<Sparkles className="h-3 w-3 text-[var(--vellum-success)]" />}
            tone="success"
          />
          <CompactStatBar
            label="魔力 MP"
            current={investigator.mp}
            max={investigator.mpMax}
            icon={<Zap className="h-3 w-3 text-[var(--vellum-accent)]" />}
            tone="accent"
          />
        </div>

        {visibleClocks.length > 0 ? (
          <div className="mt-3 space-y-2 border-t border-[var(--vellum-wood-border)]/60 pt-2.5">
            {visibleClocks.map((clock) => (
              <div key={clock.clockId}>
                <div className="flex items-center justify-between gap-2 text-[11px]">
                  <span className="inline-flex items-center gap-1 font-semibold text-[var(--vellum-ink)]">
                    <Clock className="h-3 w-3 text-[var(--vellum-danger)]" />
                    <span>{clock.name}</span>
                  </span>
                  <span className="vellum-mono font-bold text-[var(--vellum-danger)]">
                    {clock.current}/{clock.max} · {clock.stageLabel}
                  </span>
                </div>
                <div
                  className="mt-1 grid gap-1"
                  style={{
                    gridTemplateColumns: `repeat(${Math.max(1, clock.max)}, minmax(0, 1fr))`,
                  }}
                  aria-label={`${clock.name} 进度 ${clock.current} / ${clock.max}`}
                >
                  {Array.from({ length: Math.max(1, clock.max) }).map((_, idx) => (
                    <span
                      key={idx}
                      className={`h-1.5 rounded-xs ${
                        idx < clock.current
                          ? "bg-[var(--vellum-danger)]"
                          : "bg-[var(--vellum-border)]"
                      }`}
                    />
                  ))}
                </div>
              </div>
            ))}
          </div>
        ) : null}
      </section>

      {/* 2. Manor Battlefield Floorplan & Present Interactive Stage Entities */}
      <ManorBattlefieldMap
        currentScene={currentScene}
        mapRooms={mapRooms}
        actionLocked={actionLocked}
        onExecuteAction={onExecuteAction}
      />

      {/* 3. Tactile Felt-Green d100 Dice Tray */}
      <DiceTrayWidget
        pendingInterrupt={pendingInterrupt}
        lastRoll={lastRoll}
        busy={isResolvingInterrupt}
        onResolveInterrupt={onResolveInterrupt}
      />

      {/* 4. Quick-Fill Exits & Collapsible Full Investigator Sheet */}
      <section
        aria-label="调查员技能与行囊"
        className="rounded-[var(--vellum-radius-lg)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)]/90 p-3"
      >
        {currentScene.exits.some((e) => e.accessible) ? (
          <div className="mb-2.5 border-b border-[var(--vellum-border)] pb-2.5">
            <div className="text-[11px] font-semibold text-[var(--vellum-muted)]">
              填入移动指令（可先编辑再发）
            </div>
            <div className="mt-1.5 flex flex-wrap gap-1.5">
              {currentScene.exits
                .filter((exit) => exit.accessible)
                .map((exit) => (
                  <button
                    key={exit.targetSceneId}
                    type="button"
                    disabled={actionLocked}
                    onClick={() => onQuickFillAction(formatExitAction(exit.label))}
                    className="inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-bg)] px-2 py-1 text-xs text-[var(--vellum-ink)] transition-colors hover:border-[var(--vellum-accent)] hover:text-[var(--vellum-accent)] disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    <span>{exit.label}</span>
                    <ArrowUpRight className="h-3 w-3" />
                  </button>
                ))}
            </div>
          </div>
        ) : null}

        <button
          type="button"
          onClick={() => setDossierExpanded((v) => !v)}
          aria-expanded={dossierExpanded}
          className="flex w-full items-center justify-between text-xs font-semibold text-[var(--vellum-ink)] hover:text-[var(--vellum-accent)]"
        >
          <span className="inline-flex items-center gap-1.5">
            <Backpack className="h-3.5 w-3.5 text-[var(--vellum-brass)]" />
            <span>属性、技能与随身物品（{sortedSkills.length} 项技能）</span>
          </span>
          {dossierExpanded ? (
            <ChevronUp className="h-3.5 w-3.5" />
          ) : (
            <ChevronDown className="h-3.5 w-3.5" />
          )}
        </button>

        {dossierExpanded ? (
          <div className="mt-3 space-y-3">
            {characteristics.length > 0 ? (
              <div className="grid grid-cols-4 gap-1 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-elevated)]/60 p-2 text-center">
                {characteristics.map(([key, val]) => (
                  <div key={key}>
                    <div className="text-[10px] text-[var(--vellum-muted)]">
                      {formatKeyLabel(key, CHARACTERISTIC_LABELS)}
                    </div>
                    <div className="vellum-mono text-xs font-semibold text-[var(--vellum-ink)]">
                      {val}
                    </div>
                  </div>
                ))}
              </div>
            ) : null}

            <div className="grid grid-cols-2 gap-1.5">
              {sortedSkills.map(([skill, rating]) => (
                <div
                  key={skill}
                  className="flex items-center justify-between rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-bg)]/80 px-2 py-1"
                >
                  <span className="truncate text-[var(--vellum-ink)]">
                    {formatKeyLabel(skill, SKILL_LABELS)}
                  </span>
                  <span className="vellum-mono font-semibold text-[var(--vellum-brass)]">
                    {rating}%
                  </span>
                </div>
              ))}
            </div>

            {investigator.inventory.length > 0 ? (
              <ul className="space-y-1.5 border-t border-[var(--vellum-border)] pt-2.5">
                {investigator.inventory.map((item) => (
                  <li
                    key={item.itemId}
                    className="rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-bg)]/80 px-2.5 py-1.5"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="font-medium text-[var(--vellum-ink)]">{item.name}</span>
                      {item.quantity > 1 ? (
                        <span className="vellum-mono text-[11px] text-[var(--vellum-muted)]">
                          ×{item.quantity}
                        </span>
                      ) : null}
                    </div>
                    {item.description ? (
                      <p className="mt-0.5 text-[11px] leading-4 text-[var(--vellum-muted)]">
                        {item.description}
                      </p>
                    ) : null}
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        ) : (
          <div className="mt-2 flex flex-wrap gap-1.5">
            {sortedSkills.slice(0, 4).map(([skill, rating]) => (
              <span
                key={skill}
                className="inline-flex items-center gap-1 rounded bg-[var(--vellum-elevated)] px-2 py-0.5 text-[11px] text-[var(--vellum-ink)]"
              >
                <span>{formatKeyLabel(skill, SKILL_LABELS)}</span>
                <span className="vellum-mono font-semibold text-[var(--vellum-brass)]">
                  {rating}%
                </span>
              </span>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}

export function TabletopBattlefieldPane({
  investigator,
  currentScene,
  mapRooms,
  visibleClocks,
  pendingInterrupt,
  lastRoll,
  isResolvingInterrupt,
  mobileDrawerOpen,
  actionLocked = false,
  onCloseMobileDrawer,
  onExecuteAction,
  onQuickFillAction,
  onResolveInterrupt,
}: TabletopBattlefieldPaneProps) {
  useEffect(() => {
    if (!mobileDrawerOpen) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCloseMobileDrawer();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [mobileDrawerOpen, onCloseMobileDrawer]);

  const handleMobileExecute = (text: string) => {
    onExecuteAction(text);
    if (mobileDrawerOpen) onCloseMobileDrawer();
  };

  const handleMobileQuickFill = (text: string) => {
    onQuickFillAction(text);
    if (mobileDrawerOpen) onCloseMobileDrawer();
  };

  return (
    <>
      <aside
        aria-label="跑团桌面与庄园沙盘"
        className="hidden w-[408px] shrink-0 border-r border-[var(--vellum-wood-border)]/60 bg-[var(--vellum-surface)]/75 lg:block xl:w-[436px]"
      >
        <div className="sticky top-[53px] max-h-[calc(100vh-53px)] overflow-y-auto">
          <TabletopBody
            investigator={investigator}
            currentScene={currentScene}
            mapRooms={mapRooms}
            visibleClocks={visibleClocks}
            pendingInterrupt={pendingInterrupt}
            lastRoll={lastRoll}
            isResolvingInterrupt={isResolvingInterrupt}
            actionLocked={actionLocked}
            onExecuteAction={onExecuteAction}
            onQuickFillAction={onQuickFillAction}
            onResolveInterrupt={onResolveInterrupt}
          />
        </div>
      </aside>

      {mobileDrawerOpen ? (
        <div
          className="fixed inset-0 z-40 flex justify-start lg:hidden"
          role="dialog"
          aria-modal="true"
          aria-label="沙盘与调查员档案抽屉"
        >
          <div
            className="fixed inset-0 bg-black/55"
            aria-hidden="true"
            onClick={onCloseMobileDrawer}
          />
          <div className="relative z-10 flex h-full w-full max-w-[360px] flex-col border-r border-[var(--vellum-wood-border)] bg-[var(--vellum-bg)] shadow-[var(--vellum-shadow-md)]">
            <div className="flex items-center justify-between border-b border-[var(--vellum-border)] bg-[var(--vellum-surface)] px-4 py-3">
              <span className="text-sm font-semibold text-[var(--vellum-ink)]">
                庄园沙盘与骰盘
              </span>
              <button
                type="button"
                aria-label="关闭沙盘抽屉"
                onClick={onCloseMobileDrawer}
                className="rounded-[var(--vellum-radius-sm)] p-1 text-[var(--vellum-muted)] hover:bg-[var(--vellum-elevated)] hover:text-[var(--vellum-ink)]"
              >
                <X className="h-4 w-4" />
              </button>
            </div>
            <div className="flex-1 overflow-y-auto">
              <TabletopBody
                investigator={investigator}
                currentScene={currentScene}
                mapRooms={mapRooms}
                visibleClocks={visibleClocks}
                pendingInterrupt={pendingInterrupt}
                lastRoll={lastRoll}
                isResolvingInterrupt={isResolvingInterrupt}
                actionLocked={actionLocked}
                onExecuteAction={handleMobileExecute}
                onQuickFillAction={handleMobileQuickFill}
                onResolveInterrupt={onResolveInterrupt}
              />
            </div>
          </div>
        </div>
      ) : null}
    </>
  );
}
