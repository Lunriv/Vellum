import { useEffect, useState, type ReactNode } from "react";
import {
  ArrowUpRight,
  Backpack,
  ChevronDown,
  ChevronUp,
  Clock,
  Eye,
  Heart,
  Lock,
  MapPin,
  ShieldAlert,
  Sparkles,
  Users,
  X,
  Zap,
} from "lucide-react";
import type {
  InvestigatorStateDTO,
  VisibleClockDTO,
  VisibleSceneDTO,
} from "../../api/types";

interface CharacterDossierProps {
  investigator: InvestigatorStateDTO;
  currentScene: VisibleSceneDTO;
  visibleClocks: VisibleClockDTO[];
  mobileDrawerOpen: boolean;
  onCloseMobileDrawer: () => void;
  onQuickFillAction: (actionText: string) => void;
  actionLocked?: boolean;
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
  fighting_brawl: "斗殴",
  dodge: "闪避",
  firearms_handgun: "手枪",
  credit_rating: "信用评级",
  cthulhu_mythos: "克苏鲁神话",
};

const ENTITY_ROLE_LABELS: Record<string, string> = {
  gatekeeper: "守门人",
  host: "庄园主人",
  witness: "目击者",
  suspect: "相关人物",
  ally: "协助者",
  npc: "在场人物",
  object: "可查物件",
};

function formatExitAction(label: string): string {
  return /^(前往|返回|回到|推门|从|绕|侧门|进入|离开)/.test(label) ? label : `前往${label}`;
}

function formatKeyLabel(key: string, map: Record<string, string>): string {
  const exact = map[key] ?? map[key.toUpperCase()] ?? map[key.toLowerCase()];
  if (exact) return exact;
  return key.replace(/_/g, " ");
}

interface ResourceMeterProps {
  label: string;
  current: number;
  max: number;
  statusNote: string;
  icon: ReactNode;
  tone: "danger" | "accent" | "warning" | "success";
}

function ResourceMeter({ label, current, max, statusNote, icon, tone }: ResourceMeterProps) {
  const safeMax = Math.max(1, max);
  const pct = Math.max(0, Math.min(100, Math.round((current / safeMax) * 100)));

  const barToneClass =
    tone === "danger"
      ? "bg-[var(--vellum-danger)]"
      : tone === "warning"
        ? "bg-[var(--vellum-warning)]"
        : tone === "success"
          ? "bg-[var(--vellum-success)]"
          : "bg-[var(--vellum-accent)]";

  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between gap-2 text-xs">
        <span className="inline-flex items-center gap-1.5 font-medium text-[var(--vellum-ink)]">
          {icon}
          <span>{label}</span>
          <span className="text-[11px] font-normal text-[var(--vellum-muted)]">· {statusNote}</span>
        </span>
        <span className="vellum-mono font-semibold text-[var(--vellum-ink)]">
          {current} / {max}
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
        <div
          className={`h-full transition-all duration-200 ${barToneClass}`}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}

function DossierBody({
  investigator,
  currentScene,
  visibleClocks,
  onQuickFillAction,
  actionLocked = false,
}: Omit<CharacterDossierProps, "mobileDrawerOpen" | "onCloseMobileDrawer">) {
  const [skillsExpanded, setSkillsExpanded] = useState(false);

  const sortedSkills = Object.entries(investigator.skills).sort((a, b) => b[1] - a[1]);
  const visibleSkills = skillsExpanded ? sortedSkills : sortedSkills.slice(0, 6);
  const characteristics = Object.entries(investigator.characteristics);

  const hpLow = investigator.hp <= Math.max(2, Math.floor(investigator.hpMax / 3));
  const sanLow = investigator.san <= Math.min(35, Math.floor(investigator.sanMax * 0.55));

  return (
    <div className="space-y-5 p-4 text-xs">
      <section aria-label="调查员状态">
        <div className="border-b border-[var(--vellum-border)] pb-2.5">
          <h2 className="vellum-prose text-base font-semibold text-[var(--vellum-ink)]">
            {investigator.name}
          </h2>
          <p className="mt-0.5 text-xs text-[var(--vellum-muted)]">{investigator.occupation}</p>
        </div>

        <div className="mt-3 space-y-2.5">
          <ResourceMeter
            label="生命 HP"
            current={investigator.hp}
            max={investigator.hpMax}
            statusNote={hpLow ? "负伤" : "稳健"}
            icon={<Heart className="h-3.5 w-3.5 text-[var(--vellum-danger)]" />}
            tone="danger"
          />
          <ResourceMeter
            label="魔力 MP"
            current={investigator.mp}
            max={investigator.mpMax}
            statusNote={investigator.mp <= 2 ? "枯竭" : "充沛"}
            icon={<Zap className="h-3.5 w-3.5 text-[var(--vellum-accent)]" />}
            tone="accent"
          />
          <ResourceMeter
            label="理智 SAN"
            current={investigator.san}
            max={investigator.sanMax}
            statusNote={sanLow ? "紧绷" : "清明"}
            icon={<Eye className="h-3.5 w-3.5 text-[var(--vellum-warning)]" />}
            tone="warning"
          />
          <ResourceMeter
            label="幸运 Luck"
            current={investigator.luck}
            max={investigator.luckMax}
            statusNote={investigator.luck < 20 ? "匮乏" : "可用"}
            icon={<Sparkles className="h-3.5 w-3.5 text-[var(--vellum-success)]" />}
            tone="success"
          />
        </div>

        <div className="mt-3">
          {investigator.conditions.length > 0 ? (
            <div className="flex flex-wrap gap-1.5" aria-label="当前状态标签">
              {investigator.conditions.map((cond) => (
                <span
                  key={cond}
                  className="inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-warning)]/40 bg-[var(--vellum-warning-soft)] px-2 py-0.5 text-[11px] font-medium text-[var(--vellum-ink)]"
                >
                  <ShieldAlert className="h-3 w-3 text-[var(--vellum-warning)]" />
                  <span>{cond}</span>
                </span>
              ))}
            </div>
          ) : (
            <p className="text-[11px] text-[var(--vellum-muted)]">当前身心状态：平稳</p>
          )}
        </div>
      </section>

      {visibleClocks.length > 0 ? (
        <section className="border-t border-[var(--vellum-border)] pt-4" aria-label="可见局势进度">
          <h3 className="inline-flex items-center gap-1.5 font-semibold text-[var(--vellum-ink)]">
            <Clock className="h-3.5 w-3.5 text-[var(--vellum-accent)]" />
            <span>局势进度钟</span>
          </h3>
          <div className="mt-2.5 space-y-3">
            {visibleClocks.map((clock) => (
              <div
                key={clock.clockId}
                className="rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-elevated)]/60 p-2.5"
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="font-medium text-[var(--vellum-ink)]">{clock.name}</span>
                  <span className="vellum-mono font-semibold text-[var(--vellum-accent)]">
                    {clock.current} / {clock.max}
                  </span>
                </div>
                <div
                  className="mt-1.5 grid gap-1"
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
                          ? "bg-[var(--vellum-accent)]"
                          : "bg-[var(--vellum-border)]"
                      }`}
                    />
                  ))}
                </div>
                <p className="mt-1.5 text-[11px] font-medium text-[var(--vellum-ink)]">
                  {clock.stageLabel}
                </p>
                {clock.lastReason ? (
                  <p className="mt-0.5 text-[11px] text-[var(--vellum-muted)]">
                    最近动向：{clock.lastReason}
                  </p>
                ) : null}
              </div>
            ))}
          </div>
        </section>
      ) : null}

      <section className="border-t border-[var(--vellum-border)] pt-4" aria-label="当前场景概览">
        <div className="inline-flex items-center gap-1.5 font-semibold text-[var(--vellum-ink)]">
          <MapPin className="h-3.5 w-3.5 text-[var(--vellum-accent)]" />
          <span>当前位置 · {currentScene.title}</span>
        </div>
        {currentScene.description ? (
          <p className="mt-1.5 leading-5 text-[var(--vellum-muted)]">{currentScene.description}</p>
        ) : null}

        {currentScene.exits.length > 0 ? (
          <div className="mt-3">
            <div className="text-[11px] font-semibold text-[var(--vellum-muted)]">可前往地点</div>
            <div className="mt-1.5 flex flex-wrap gap-1.5">
              {currentScene.exits.map((exit) =>
                exit.accessible ? (
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
                ) : (
                  <span
                    key={exit.targetSceneId}
                    title={exit.blockedReason ?? "当前无法通行"}
                    className="inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-elevated)] px-2 py-1 text-[11px] text-[var(--vellum-muted)]"
                  >
                    <Lock className="h-3 w-3" />
                    <span>
                      {exit.label}
                      {exit.blockedReason ? `（${exit.blockedReason}）` : "（暂不可通行）"}
                    </span>
                  </span>
                ),
              )}
            </div>
          </div>
        ) : null}

        {currentScene.presentEntities.length > 0 ? (
          <div className="mt-3">
            <div className="inline-flex items-center gap-1 text-[11px] font-semibold text-[var(--vellum-muted)]">
              <Users className="h-3 w-3" />
              <span>在场人物与可查物件</span>
            </div>
            <ul className="mt-1.5 space-y-1.5">
              {currentScene.presentEntities.map((entity) => (
                <li
                  key={entity.entityId}
                  className="rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-bg)]/70 px-2.5 py-1.5"
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="font-medium text-[var(--vellum-ink)]">{entity.name}</span>
                    <span className="text-[10px] text-[var(--vellum-muted)]">
                      {entity.role
                        ? formatKeyLabel(entity.role, ENTITY_ROLE_LABELS)
                        : entity.kind === "npc"
                          ? "在场人物"
                          : "可查物件"}
                    </span>
                  </div>
                  {entity.summary ? (
                    <p className="mt-0.5 leading-5 text-[var(--vellum-muted)]">{entity.summary}</p>
                  ) : null}
                </li>
              ))}
            </ul>
          </div>
        ) : null}
      </section>

      <section className="border-t border-[var(--vellum-border)] pt-4" aria-label="调查员技能与属性">
        {characteristics.length > 0 ? (
          <div className="mb-3 grid grid-cols-4 gap-1 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-elevated)]/50 p-2 text-center">
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

        <div className="flex items-center justify-between">
          <h3 className="font-semibold text-[var(--vellum-ink)]">调查技能</h3>
          {sortedSkills.length > 6 ? (
            <button
              type="button"
              onClick={() => setSkillsExpanded((v) => !v)}
              className="inline-flex items-center gap-0.5 text-[11px] font-medium text-[var(--vellum-accent)] hover:underline"
            >
              <span>{skillsExpanded ? "收起" : `全部 (${sortedSkills.length})`}</span>
              {skillsExpanded ? (
                <ChevronUp className="h-3 w-3" />
              ) : (
                <ChevronDown className="h-3 w-3" />
              )}
            </button>
          ) : null}
        </div>

        <div className="mt-2 grid grid-cols-2 gap-1.5">
          {visibleSkills.map(([skill, rating]) => (
            <div
              key={skill}
              className="flex items-center justify-between rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-bg)]/70 px-2 py-1"
            >
              <span className="truncate text-[var(--vellum-ink)]">
                {formatKeyLabel(skill, SKILL_LABELS)}
              </span>
              <span className="vellum-mono font-semibold text-[var(--vellum-muted)]">
                {rating}%
              </span>
            </div>
          ))}
        </div>
      </section>

      <section className="border-t border-[var(--vellum-border)] pt-4" aria-label="随身物品">
        <h3 className="inline-flex items-center gap-1.5 font-semibold text-[var(--vellum-ink)]">
          <Backpack className="h-3.5 w-3.5 text-[var(--vellum-accent)]" />
          <span>随身物品</span>
        </h3>
        {investigator.inventory.length > 0 ? (
          <ul className="mt-2 space-y-1.5">
            {investigator.inventory.map((item) => (
              <li
                key={item.itemId}
                className="rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-bg)]/70 px-2.5 py-1.5"
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
                  <p className="mt-0.5 leading-5 text-[var(--vellum-muted)]">{item.description}</p>
                ) : null}
              </li>
            ))}
          </ul>
        ) : (
          <p className="mt-1.5 text-[var(--vellum-muted)]">行囊中暂无物品。</p>
        )}
      </section>
    </div>
  );
}

export function CharacterDossier({
  investigator,
  currentScene,
  visibleClocks,
  mobileDrawerOpen,
  onCloseMobileDrawer,
  onQuickFillAction,
  actionLocked = false,
}: CharacterDossierProps) {
  useEffect(() => {
    if (!mobileDrawerOpen) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCloseMobileDrawer();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [mobileDrawerOpen, onCloseMobileDrawer]);

  const handleQuickFill = (text: string) => {
    onQuickFillAction(text);
    if (mobileDrawerOpen) {
      onCloseMobileDrawer();
    }
  };

  return (
    <>
      <aside
        aria-label="调查员档案与现场边栏"
        className="hidden w-[290px] shrink-0 border-l border-[var(--vellum-border)] bg-[var(--vellum-surface)]/85 lg:block"
      >
        <div className="sticky top-[53px] max-h-[calc(100vh-53px)] overflow-y-auto">
          <DossierBody
            investigator={investigator}
            currentScene={currentScene}
            visibleClocks={visibleClocks}
            onQuickFillAction={handleQuickFill}
            actionLocked={actionLocked}
          />
        </div>
      </aside>

      {mobileDrawerOpen ? (
        <div
          className="fixed inset-0 z-40 flex justify-end lg:hidden"
          role="dialog"
          aria-modal="true"
          aria-label="调查员档案抽屉"
        >
          <div
            className="fixed inset-0 bg-black/45"
            aria-hidden="true"
            onClick={onCloseMobileDrawer}
          />
          <div className="relative z-10 flex h-full w-full max-w-[320px] flex-col border-l border-[var(--vellum-border)] bg-[var(--vellum-surface)] shadow-[var(--vellum-shadow-md)]">
            <div className="flex items-center justify-between border-b border-[var(--vellum-border)] px-4 py-3">
              <span className="text-sm font-semibold text-[var(--vellum-ink)]">调查员档案</span>
              <button
                type="button"
                aria-label="关闭调查员档案"
                onClick={onCloseMobileDrawer}
                className="rounded-[var(--vellum-radius-sm)] p-1 text-[var(--vellum-muted)] hover:bg-[var(--vellum-elevated)] hover:text-[var(--vellum-ink)]"
              >
                <X className="h-4 w-4" />
              </button>
            </div>
            <div className="flex-1 overflow-y-auto">
              <DossierBody
                investigator={investigator}
                currentScene={currentScene}
                visibleClocks={visibleClocks}
                onQuickFillAction={handleQuickFill}
                actionLocked={actionLocked}
              />
            </div>
          </div>
        </div>
      ) : null}
    </>
  );
}
