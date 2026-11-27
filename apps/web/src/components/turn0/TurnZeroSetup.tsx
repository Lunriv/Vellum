import { useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  AlertCircle,
  ArrowRight,
  Briefcase,
  Compass,
  Dices,
  Eye,
  Heart,
  RefreshCw,
  Sparkles,
  Zap,
} from "lucide-react";
import { createIdempotencyKey, vellumApi } from "../../api/client";
import type {
  CreateSessionResponse,
  ExperienceMode,
  InvestigatorPresetDTO,
} from "../../api/types";
import { useGameStore } from "../../stores/gameStore";

const MODULE_ID = "crimson_rain_manor";

interface TurnZeroSetupProps {
  onSessionCreated: (response: CreateSessionResponse) => void;
}

const MODE_CARDS: Array<{
  value: ExperienceMode;
  label: string;
  badge?: string;
  description: string;
}> = [
  {
    value: "guided",
    label: "引导",
    description: "全程保留守秘人行动选项与检定风险提示，新手可一键轻松推进。",
  },
  {
    value: "classic",
    label: "经典",
    badge: "推荐",
    description: "沙盘、骰盘与多声道剧场完整联动，保留 3 项即刻回应与自由输入。",
  },
  {
    value: "immersive",
    label: "沉浸",
    description: "隐藏快捷选项，仅保留手写自由扮演与实体骰盘，还原纯桌游体验。",
  },
];

function investigatorTone(id: string): string {
  if (id.includes("reporter")) return "#9d3338";
  if (id.includes("detective")) return "#2c546e";
  return "#2a6249";
}

export function TurnZeroSetup({ onSessionCreated }: TurnZeroSetupProps) {
  const experienceMode = useGameStore((s) => s.experienceMode);
  const setExperienceMode = useGameStore((s) => s.setExperienceMode);

  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const submittingLockRef = useRef(false);
  const idempotencyKeyRef = useRef<string>(createIdempotencyKey());

  const {
    data,
    isLoading,
    isError,
    error,
    refetch,
    isFetching,
  } = useQuery({
    queryKey: ["investigators", MODULE_ID],
    queryFn: () => vellumApi.listInvestigators(MODULE_ID),
  });

  const investigators: InvestigatorPresetDTO[] = data?.investigators ?? [];
  const activeInvestigatorId = selectedId ?? investigators[0]?.investigatorId ?? null;

  const handleStartAdventure = async () => {
    if (!activeInvestigatorId || submittingLockRef.current) return;
    submittingLockRef.current = true;
    setSubmitting(true);
    setSubmitError(null);

    try {
      const res = await vellumApi.createSession(
        {
          moduleId: MODULE_ID,
          investigatorId: activeInvestigatorId,
          experienceMode,
        },
        idempotencyKeyRef.current,
      );
      onSessionCreated(res);
    } catch (err) {
      idempotencyKeyRef.current = createIdempotencyKey();
      setSubmitError(
        err instanceof Error && err.message
          ? `暂时无法开启调查：${err.message}`
          : "暂时无法连接守秘人服务，请检查服务是否已启动后重试。",
      );
      submittingLockRef.current = false;
      setSubmitting(false);
    }
  };

  return (
    <main className="mx-auto max-w-6xl px-4 py-8 sm:px-6 sm:py-10">
      <header className="vellum-wood-panel relative overflow-hidden rounded-[var(--vellum-radius-lg)] p-6 sm:p-8">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="max-w-2xl">
            <p className="inline-flex items-center gap-1.5 text-xs font-bold tracking-[0.18em] text-[var(--vellum-brass)]">
              <Dices className="h-3.5 w-3.5" />
              <span>克苏鲁跑团桌 · 1927 调查短篇</span>
            </p>
            <h1 className="vellum-prose mt-2 text-2xl font-bold text-[var(--vellum-ink)] sm:text-3xl">
              {data?.moduleTitle ?? "绯雨庄园"}
            </h1>
            <p className="mt-2.5 text-sm leading-7 text-[var(--vellum-muted)]">
              连绵暴雨封住了通往山脊的旧道，一封无署名委托函将你引向这座秘而不宣的庄园。桌面沙盘已铺开，守秘人与庄园里的每一位知情者都在灯影下等着你的第一句发问。
            </p>
          </div>
          <div className="hidden rounded-[var(--vellum-radius-md)] border border-[var(--vellum-wood-border)] bg-[var(--vellum-felt-bg)] px-4 py-3 text-right text-xs text-[#f4ead4] sm:block">
            <div className="font-semibold text-[#f0d18c]">本桌配置</div>
            <div className="mt-1 opacity-90">羊皮卷庄园沙盘 · 实体 d100 骰盘</div>
            <div className="mt-0.5 opacity-90">多声道实时对戏 · 确定性内核裁定</div>
          </div>
        </div>
      </header>

      {isLoading ? (
        <section
          aria-live="polite"
          className="mt-6 rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] p-6 shadow-[var(--vellum-shadow-sm)]"
        >
          <p className="text-sm font-medium text-[var(--vellum-ink)]">正在铺开调查员角色卡……</p>
          <div className="mt-4 grid gap-4 md:grid-cols-3">
            {[0, 1, 2].map((idx) => (
              <div
                key={idx}
                className="h-64 rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-elevated)]/60"
              />
            ))}
          </div>
        </section>
      ) : null}

      {isError ? (
        <section
          role="alert"
          className="mt-6 rounded-[var(--vellum-radius-md)] border border-[var(--vellum-danger)]/40 bg-[var(--vellum-danger-soft)] px-5 py-4 text-[var(--vellum-ink)]"
        >
          <div className="flex items-start gap-3">
            <AlertCircle className="mt-0.5 h-5 w-5 shrink-0 text-[var(--vellum-danger)]" />
            <div className="flex-1">
              <h2 className="text-sm font-semibold">无法读取调查员档案</h2>
              <p className="mt-1 text-xs leading-6 text-[var(--vellum-muted)]">
                {error instanceof Error && error.message
                  ? `连接未能完成（${error.message}）。请确认本地服务已运行，随后点击重试。`
                  : "当前无法连接后端服务，请稍后重试。"}
              </p>
              <button
                type="button"
                disabled={isFetching}
                onClick={() => void refetch()}
                className="mt-3 inline-flex items-center gap-1.5 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border-strong)] bg-[var(--vellum-surface)] px-3 py-1.5 text-xs font-medium text-[var(--vellum-ink)] hover:bg-[var(--vellum-elevated)] disabled:opacity-50"
              >
                <RefreshCw className="h-3.5 w-3.5" />
                <span>{isFetching ? "正在重试…" : "重新加载"}</span>
              </button>
            </div>
          </div>
        </section>
      ) : null}

      {!isLoading && !isError && investigators.length > 0 ? (
        <>
          <section className="mt-7" aria-labelledby="investigator-heading">
            <div className="flex items-baseline justify-between gap-2">
              <h2 id="investigator-heading" className="text-sm font-bold text-[var(--vellum-ink)]">
                选择调查员身份
              </h2>
              <span className="text-xs text-[var(--vellum-muted)]">
                共 {investigators.length} 张可扮演角色卡
              </span>
            </div>

            <div className="mt-3 grid gap-4 lg:grid-cols-3">
              {investigators.map((inv) => {
                const selected = inv.investigatorId === activeInvestigatorId;
                return (
                  <article
                    key={inv.investigatorId}
                    onClick={() => setSelectedId(inv.investigatorId)}
                    className={`flex cursor-pointer flex-col justify-between rounded-[var(--vellum-radius-lg)] border p-5 transition-all ${
                      selected
                        ? "border-[var(--vellum-accent)] bg-[var(--vellum-surface)] ring-1 ring-[var(--vellum-accent)]/60 shadow-[var(--vellum-shadow-md)]"
                        : "border-[var(--vellum-wood-border)]/70 bg-[var(--vellum-surface)]/85 hover:border-[var(--vellum-brass)]"
                    }`}
                  >
                    <div>
                      <div className="flex items-start justify-between gap-2">
                        <div className="flex items-center gap-3">
                          <div
                            className="vellum-prose flex h-11 w-11 shrink-0 items-center justify-center rounded-full border-2 border-[var(--vellum-brass)] text-base font-bold text-[#fff8eb] shadow-xs"
                            style={{ backgroundColor: investigatorTone(inv.investigatorId) }}
                            aria-hidden="true"
                          >
                            {inv.name.slice(0, 1)}
                          </div>
                          <div>
                            <h3 className="vellum-prose text-lg font-bold text-[var(--vellum-ink)]">
                              {inv.name}
                            </h3>
                            <p className="mt-0.5 inline-flex items-center gap-1.5 text-xs font-medium text-[var(--vellum-brass)]">
                              <Briefcase className="h-3.5 w-3.5" />
                              <span>{inv.occupation}</span>
                            </p>
                          </div>
                        </div>
                        <button
                          type="button"
                          aria-pressed={selected}
                          onClick={(e) => {
                            e.stopPropagation();
                            setSelectedId(inv.investigatorId);
                          }}
                          className={`rounded-[var(--vellum-radius-sm)] px-2.5 py-1 text-xs font-semibold ${
                            selected
                              ? "bg-[var(--vellum-accent)] text-[var(--vellum-accent-ink)]"
                              : "border border-[var(--vellum-border)] bg-[var(--vellum-elevated)] text-[var(--vellum-muted)] hover:text-[var(--vellum-ink)]"
                          }`}
                        >
                          {selected ? "已选定" : "选择"}
                        </button>
                      </div>

                      <p className="mt-3 border-l-2 border-[var(--vellum-accent)]/70 pl-3 text-xs font-medium leading-6 text-[var(--vellum-ink)]">
                        {inv.tagline}
                      </p>

                      <p className="mt-2.5 text-xs leading-6 text-[var(--vellum-muted)]">{inv.biography}</p>

                      <dl className="mt-3 space-y-2 border-t border-[var(--vellum-border)] pt-3 text-xs">
                        <div>
                          <dt className="font-semibold text-[var(--vellum-ink)]">此行动机</dt>
                          <dd className="mt-0.5 leading-5 text-[var(--vellum-muted)]">{inv.motivation}</dd>
                        </div>
                        <div>
                          <dt className="font-semibold text-[var(--vellum-ink)]">与庄园的联系</dt>
                          <dd className="mt-0.5 leading-5 text-[var(--vellum-muted)]">
                            {inv.manorConnection}
                          </dd>
                        </div>
                      </dl>

                      <div className="mt-3 grid grid-cols-4 gap-1.5 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-elevated)]/70 p-2 text-center">
                        <div>
                          <div className="inline-flex items-center justify-center gap-1 text-[11px] text-[var(--vellum-muted)]">
                            <Heart className="h-3 w-3 text-[var(--vellum-danger)]" />
                            <span>生命</span>
                          </div>
                          <div className="vellum-mono text-xs font-semibold text-[var(--vellum-ink)]">
                            {inv.hpMax}
                          </div>
                        </div>
                        <div>
                          <div className="inline-flex items-center justify-center gap-1 text-[11px] text-[var(--vellum-muted)]">
                            <Zap className="h-3 w-3 text-[var(--vellum-accent)]" />
                            <span>魔力</span>
                          </div>
                          <div className="vellum-mono text-xs font-semibold text-[var(--vellum-ink)]">
                            {inv.mpMax}
                          </div>
                        </div>
                        <div>
                          <div className="inline-flex items-center justify-center gap-1 text-[11px] text-[var(--vellum-muted)]">
                            <Eye className="h-3 w-3 text-[var(--vellum-warning)]" />
                            <span>理智</span>
                          </div>
                          <div className="vellum-mono text-xs font-semibold text-[var(--vellum-ink)]">
                            {inv.sanStart}
                          </div>
                        </div>
                        <div>
                          <div className="inline-flex items-center justify-center gap-1 text-[11px] text-[var(--vellum-muted)]">
                            <Sparkles className="h-3 w-3 text-[var(--vellum-success)]" />
                            <span>幸运</span>
                          </div>
                          <div className="vellum-mono text-xs font-semibold text-[var(--vellum-ink)]">
                            {inv.luckStart}
                          </div>
                        </div>
                      </div>

                      <div className="mt-3">
                        <div className="text-xs font-semibold text-[var(--vellum-ink)]">核心专长</div>
                        <div className="mt-1.5 flex flex-wrap gap-1.5">
                          {inv.strengths.map((s) => (
                            <span
                              key={s.skill}
                              className="inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-elevated)] px-2 py-0.5 text-[11px] text-[var(--vellum-ink)]"
                            >
                              <span>{s.label}</span>
                              <span className="vellum-mono font-semibold text-[var(--vellum-brass)]">
                                {s.rating}%
                              </span>
                            </span>
                          ))}
                        </div>
                      </div>
                    </div>

                    <div className="mt-4 border-t border-[var(--vellum-border)] pt-3">
                      <div className="inline-flex items-center gap-1 text-xs font-semibold text-[var(--vellum-ink)]">
                        <Compass className="h-3.5 w-3.5 text-[var(--vellum-brass)]" />
                        <span>随身物品</span>
                      </div>
                      <ul className="mt-1.5 space-y-1 text-xs text-[var(--vellum-muted)]">
                        {inv.startingItems.map((item) => (
                          <li key={item.itemId}>
                            <span className="font-medium text-[var(--vellum-ink)]">{item.name}</span>
                            {item.description ? `：${item.description}` : ""}
                          </li>
                        ))}
                      </ul>
                    </div>
                  </article>
                );
              })}
            </div>
          </section>

          <section className="mt-7" aria-labelledby="experience-mode-heading">
            <h2 id="experience-mode-heading" className="text-sm font-bold text-[var(--vellum-ink)]">
              选择跑团交互模式
            </h2>
            <p className="mt-1 text-xs text-[var(--vellum-muted)]">
              仅调整快捷回应与提示显示，不改变案件真相与检定规则，入桌后可随时在顶部切换。
            </p>

            <div className="mt-3 grid gap-3 sm:grid-cols-3" role="radiogroup" aria-label="体验模式选择">
              {MODE_CARDS.map((mode) => {
                const active = experienceMode === mode.value;
                return (
                  <button
                    key={mode.value}
                    type="button"
                    role="radio"
                    aria-checked={active}
                    onClick={() => setExperienceMode(mode.value)}
                    className={`rounded-[var(--vellum-radius-md)] border p-4 text-left transition-colors ${
                      active
                        ? "border-[var(--vellum-accent)] bg-[var(--vellum-accent-soft)]"
                        : "border-[var(--vellum-border)] bg-[var(--vellum-surface)] hover:border-[var(--vellum-brass)]"
                    }`}
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="text-sm font-semibold text-[var(--vellum-ink)]">{mode.label}</span>
                      {mode.badge ? (
                        <span className="rounded border border-[var(--vellum-brass)]/60 bg-[var(--vellum-surface)] px-1.5 py-0.5 text-[10px] font-semibold text-[var(--vellum-brass)]">
                          {mode.badge}
                        </span>
                      ) : null}
                    </div>
                    <p className="mt-1.5 text-xs leading-5 text-[var(--vellum-muted)]">
                      {mode.description}
                    </p>
                  </button>
                );
              })}
            </div>
          </section>

          {submitError ? (
            <div
              role="alert"
              className="mt-6 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-danger)]/40 bg-[var(--vellum-danger-soft)] px-4 py-3 text-xs text-[var(--vellum-ink)]"
            >
              {submitError}
            </div>
          ) : null}

          <div className="mt-7 flex flex-wrap items-center justify-between gap-4 border-t border-[var(--vellum-wood-border)]/70 pt-6">
            <p className="text-xs text-[var(--vellum-muted)]">
              入桌后守秘人将为你实时主持开篇序幕，老管家 Arthur 已提灯候在门厅。
            </p>
            <button
              type="button"
              disabled={!activeInvestigatorId || submitting}
              onClick={() => void handleStartAdventure()}
              className="inline-flex items-center gap-2 rounded-[var(--vellum-radius-md)] bg-[var(--vellum-accent)] px-6 py-2.5 text-sm font-bold text-[var(--vellum-accent-ink)] shadow-[var(--vellum-shadow-md)] transition-colors hover:bg-[var(--vellum-accent-hover)] disabled:opacity-55"
            >
              <span>{submitting ? "正在推开庄园铁门…" : "落座开团 · 踏入绯雨庄园"}</span>
              <ArrowRight className="h-4 w-4" />
            </button>
          </div>
        </>
      ) : null}
    </main>
  );
}
