import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  AlertTriangle,
  ArrowLeft,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Clock,
  EyeOff,
  FileSearch,
  GitBranch,
  Lock,
  RefreshCw,
  Unlock,
} from "lucide-react";
import { vellumApi } from "../../api/client";

interface CaseDebriefViewProps {
  sessionId: string;
  onBackToTranscript: () => void;
  onStartNewAdventure: () => void;
}

const SECRET_TIER_LABELS: Record<string, string> = {
  tell: "一阶 · 露出破绽",
  partial: "二阶 · 部分吐露",
  confession: "三阶 · 彻底坦白",
};

function SourceEventTags({ eventIds }: { eventIds: string[] }) {
  const [expanded, setExpanded] = useState(false);

  if (eventIds.length === 0) return null;

  return (
    <div className="mt-2">
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className="inline-flex items-center gap-1 text-[11px] font-medium text-[var(--vellum-muted)] hover:text-[var(--vellum-ink)]"
      >
        <span>{expanded ? "收起来源事件编号" : `查看来源事件编号 (${eventIds.length})`}</span>
        {expanded ? <ChevronUp className="h-3 w-3" /> : <ChevronDown className="h-3 w-3" />}
      </button>
      {expanded ? (
        <div className="mt-1.5 flex flex-wrap gap-1.5">
          {eventIds.map((id) => (
            <span
              key={id}
              className="vellum-mono rounded border border-[var(--vellum-border)] bg-[var(--vellum-elevated)] px-1.5 py-0.5 text-[11px] text-[var(--vellum-muted)]"
            >
              {id}
            </span>
          ))}
        </div>
      ) : null}
    </div>
  );
}

export function CaseDebriefView({
  sessionId,
  onBackToTranscript,
  onStartNewAdventure,
}: CaseDebriefViewProps) {
  const [spoilerConfirmed, setSpoilerConfirmed] = useState(false);

  const debriefQuery = useQuery({
    queryKey: ["debrief", sessionId],
    queryFn: () => vellumApi.getDebrief(sessionId),
    enabled: spoilerConfirmed && Boolean(sessionId),
  });

  if (!spoilerConfirmed) {
    return (
      <main className="mx-auto max-w-2xl px-4 py-12 sm:px-6">
        <section
          aria-labelledby="spoiler-gate-heading"
          className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border-strong)] bg-[var(--vellum-surface)] p-6 shadow-[var(--vellum-shadow-md)] sm:p-8"
        >
          <div className="inline-flex items-center gap-2 rounded-full border border-[var(--vellum-warning)]/40 bg-[var(--vellum-warning-soft)] px-3 py-1 text-xs font-semibold text-[var(--vellum-warning)]">
            <AlertTriangle className="h-3.5 w-3.5" />
            <span>包含完整剧透</span>
          </div>

          <h1
            id="spoiler-gate-heading"
            className="vellum-prose mt-4 text-2xl font-semibold text-[var(--vellum-ink)]"
          >
            结案真相复盘卷宗
          </h1>
          <p className="mt-2.5 text-sm leading-7 text-[var(--vellum-muted)]">
            本卷宗将完整公开当前世界模组的真实事发经过、所有核心人物的隐秘动机、守秘人幕后进度钟，以及其他分支结局的达成条件。若你打算换一位调查员重玩探索未知路线，建议暂缓翻阅。
          </p>

          <div className="mt-6 flex flex-wrap items-center gap-3">
            <button
              type="button"
              onClick={() => setSpoilerConfirmed(true)}
              className="rounded-[var(--vellum-radius-sm)] bg-[var(--vellum-accent)] px-4 py-2 text-xs font-semibold text-[var(--vellum-accent-ink)] hover:bg-[var(--vellum-accent-hover)]"
            >
              确认查看完整真相
            </button>
            <button
              type="button"
              onClick={onBackToTranscript}
              className="inline-flex items-center gap-1.5 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-elevated)] px-3.5 py-2 text-xs font-medium text-[var(--vellum-ink)] hover:border-[var(--vellum-border-strong)]"
            >
              <ArrowLeft className="h-3.5 w-3.5" />
              <span>返回调查记录</span>
            </button>
          </div>
        </section>
      </main>
    );
  }

  const debrief = debriefQuery.data;

  return (
    <main className="mx-auto max-w-3xl px-4 py-8 sm:px-6 sm:py-12">
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <button
          type="button"
          onClick={onBackToTranscript}
          className="inline-flex items-center gap-1.5 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] px-3 py-1.5 text-xs font-medium text-[var(--vellum-ink)] hover:bg-[var(--vellum-elevated)]"
        >
          <ArrowLeft className="h-3.5 w-3.5" />
          <span>返回本局叙事记录</span>
        </button>

        <button
          type="button"
          onClick={onStartNewAdventure}
          className="rounded-[var(--vellum-radius-sm)] bg-[var(--vellum-accent)] px-3.5 py-1.5 text-xs font-semibold text-[var(--vellum-accent-ink)] hover:bg-[var(--vellum-accent-hover)]"
        >
          开启新的调查
        </button>
      </div>

      {debriefQuery.isLoading ? (
        <section className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] p-6 text-sm text-[var(--vellum-muted)]">
          正在整理结案真相卷宗……
        </section>
      ) : null}

      {debriefQuery.isError ? (
        <section
          role="alert"
          className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-danger)]/40 bg-[var(--vellum-danger-soft)] p-5 text-xs text-[var(--vellum-ink)]"
        >
          <div className="font-semibold">未能加载结案复盘</div>
          <p className="mt-1 text-[var(--vellum-muted)]">
            {debriefQuery.error instanceof Error
              ? debriefQuery.error.message
              : "拉取复盘数据时发生异常。"}
          </p>
          <button
            type="button"
            onClick={() => void debriefQuery.refetch()}
            className="mt-3 inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border-strong)] bg-[var(--vellum-surface)] px-3 py-1.5 font-medium text-[var(--vellum-ink)]"
          >
            <RefreshCw className="h-3 w-3" />
            <span>重试</span>
          </button>
        </section>
      ) : null}

      {debrief ? (
        <div className="space-y-8">
          <header className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border-strong)] bg-[var(--vellum-surface)] p-6 shadow-[var(--vellum-shadow-sm)] sm:p-8">
            <p className="text-xs font-semibold tracking-[0.14em] text-[var(--vellum-accent)]">
              {debrief.moduleTitle} · 结案归档
            </p>
            <h1 className="vellum-prose mt-2 text-2xl font-semibold text-[var(--vellum-ink)] sm:text-3xl">
              {debrief.endingTitle}
            </h1>
            <p className="vellum-prose mt-4 whitespace-pre-line text-[var(--vellum-ink)]">
              {debrief.endingSummary}
            </p>
          </header>

          <section aria-labelledby="impact-heading" className="space-y-3">
            <h2
              id="impact-heading"
              className="inline-flex items-center gap-2 text-base font-semibold text-[var(--vellum-ink)]"
            >
              <GitBranch className="h-4 w-4 text-[var(--vellum-accent)]" />
              <span>你的关键选择如何改变了结果</span>
            </h2>
            <div className="space-y-3">
              {debrief.playerImpact.map((item, idx) => (
                <article
                  key={idx}
                  className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] p-4"
                >
                  <div className="text-sm font-semibold text-[var(--vellum-ink)]">{item.choice}</div>
                  <p className="mt-1.5 text-xs leading-6 text-[var(--vellum-muted)]">
                    {item.consequence}
                  </p>
                  <SourceEventTags eventIds={item.sourceEventIds} />
                </article>
              ))}
            </div>
          </section>

          <section aria-labelledby="chronology-heading" className="space-y-3">
            <h2
              id="chronology-heading"
              className="inline-flex items-center gap-2 text-base font-semibold text-[var(--vellum-ink)]"
            >
              <Clock className="h-4 w-4 text-[var(--vellum-accent)]" />
              <span>事件真实时间线</span>
            </h2>
            <ol className="space-y-3 border-l-2 border-[var(--vellum-border-strong)] pl-4">
              {debrief.trueChronology.map((item, idx) => (
                <li
                  key={idx}
                  className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] p-4"
                >
                  <div className="text-sm font-semibold text-[var(--vellum-ink)]">{item.title}</div>
                  <p className="mt-1.5 text-xs leading-6 text-[var(--vellum-muted)]">
                    {item.detail}
                  </p>
                  <SourceEventTags eventIds={item.sourceEventIds} />
                </li>
              ))}
            </ol>
          </section>

          <section aria-labelledby="npc-motives-heading" className="space-y-3">
            <h2 id="npc-motives-heading" className="text-base font-semibold text-[var(--vellum-ink)]">
              核心人物动机与秘密阶段
            </h2>
            <div className="space-y-4">
              {debrief.npcMotives.map((npc) => (
                <article
                  key={npc.entityId}
                  className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] p-5"
                >
                  <h3 className="vellum-prose text-lg font-semibold text-[var(--vellum-ink)]">
                    {npc.name}
                  </h3>
                  <p className="mt-1 text-xs leading-6 text-[var(--vellum-muted)]">
                    真实动机：{npc.motive}
                  </p>
                  <div className="mt-3 space-y-2 border-t border-[var(--vellum-border)] pt-3">
                    {npc.secretStages.map((stage, idx) => (
                      <div
                        key={idx}
                        className="flex items-start gap-2.5 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-bg)]/70 p-3 text-xs"
                      >
                        {stage.unlocked ? (
                          <Unlock className="mt-0.5 h-3.5 w-3.5 shrink-0 text-[var(--vellum-success)]" />
                        ) : (
                          <Lock className="mt-0.5 h-3.5 w-3.5 shrink-0 text-[var(--vellum-muted)]" />
                        )}
                        <div className="flex-1">
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="font-semibold text-[var(--vellum-ink)]">
                              {SECRET_TIER_LABELS[stage.tier] ?? stage.tier}
                            </span>
                            <span
                              className={`rounded px-1.5 py-0.5 text-[10px] font-medium ${
                                stage.unlocked
                                  ? "bg-[var(--vellum-success-soft)] text-[var(--vellum-success)]"
                                  : "bg-[var(--vellum-elevated)] text-[var(--vellum-muted)]"
                              }`}
                            >
                              {stage.unlocked ? "本局已揭开" : "本局未触及"}
                            </span>
                          </div>
                          <p className="mt-1 leading-5 text-[var(--vellum-ink)]">{stage.truth}</p>
                        </div>
                      </div>
                    ))}
                  </div>
                </article>
              ))}
            </div>
          </section>

          <section aria-labelledby="clocks-final-heading" className="space-y-3">
            <h2 id="clocks-final-heading" className="text-base font-semibold text-[var(--vellum-ink)]">
              全部进度钟终态
            </h2>
            <div className="grid gap-3 sm:grid-cols-2">
              {debrief.finalClocks.map((clock) => {
                const isHidden = clock.visibility === "keeper_hidden";
                return (
                  <div
                    key={clock.clockId}
                    className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] p-4 text-xs"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="font-semibold text-[var(--vellum-ink)]">{clock.name}</span>
                      <span
                        className={`inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-medium ${
                          isHidden
                            ? "bg-[var(--vellum-warning-soft)] text-[var(--vellum-warning)]"
                            : "bg-[var(--vellum-elevated)] text-[var(--vellum-muted)]"
                        }`}
                      >
                        {isHidden ? <EyeOff className="h-3 w-3" /> : null}
                        <span>{isHidden ? "守秘人隐藏钟" : "玩家可见钟"}</span>
                      </span>
                    </div>

                    <div className="mt-2 flex items-center justify-between text-[11px] text-[var(--vellum-muted)]">
                      <span>{clock.stageLabel}</span>
                      <span className="vellum-mono font-semibold text-[var(--vellum-ink)]">
                        {clock.current} / {clock.max}
                      </span>
                    </div>

                    <div
                      className="mt-1.5 grid gap-1"
                      style={{
                        gridTemplateColumns: `repeat(${Math.max(1, clock.max)}, minmax(0, 1fr))`,
                      }}
                    >
                      {Array.from({ length: Math.max(1, clock.max) }).map((_, i) => (
                        <span
                          key={i}
                          className={`h-1.5 rounded-xs ${
                            i < clock.current
                              ? "bg-[var(--vellum-accent)]"
                              : "bg-[var(--vellum-border)]"
                          }`}
                        />
                      ))}
                    </div>
                  </div>
                );
              })}
            </div>
          </section>

          <section aria-labelledby="missed-clues-heading" className="space-y-3">
            <h2
              id="missed-clues-heading"
              className="inline-flex items-center gap-2 text-base font-semibold text-[var(--vellum-ink)]"
            >
              <FileSearch className="h-4 w-4 text-[var(--vellum-accent)]" />
              <span>错过的重要线索</span>
            </h2>
            {debrief.missedClues.length === 0 ? (
              <p className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] p-4 text-xs text-[var(--vellum-muted)]">
                你在本局调查中已找齐所有关键线索。
              </p>
            ) : (
              <div className="space-y-2.5">
                {debrief.missedClues.map((clue) => (
                  <article
                    key={clue.clueId}
                    className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] p-4 text-xs"
                  >
                    <h3 className="font-semibold text-[var(--vellum-ink)]">{clue.title}</h3>
                    <p className="mt-1 leading-6 text-[var(--vellum-muted)]">{clue.summary}</p>
                  </article>
                ))}
              </div>
            )}
          </section>

          <section aria-labelledby="alt-endings-heading" className="space-y-3 pb-8">
            <h2 id="alt-endings-heading" className="text-base font-semibold text-[var(--vellum-ink)]">
              其他合理结局对比
            </h2>
            <div className="space-y-2.5">
              {debrief.alternativeEndings.map((ending, idx) => (
                <article
                  key={idx}
                  className={`rounded-[var(--vellum-radius-md)] border p-4 text-xs ${
                    ending.achieved
                      ? "border-[var(--vellum-accent)] bg-[var(--vellum-accent-soft)]/40"
                      : "border-[var(--vellum-border)] bg-[var(--vellum-surface)]"
                  }`}
                >
                  <div className="flex items-center justify-between gap-2">
                    <h3 className="text-sm font-semibold text-[var(--vellum-ink)]">
                      {ending.title}
                    </h3>
                    <span
                      className={`inline-flex items-center gap-1 rounded px-2 py-0.5 text-[11px] font-medium ${
                        ending.achieved
                          ? "bg-[var(--vellum-accent)] text-[var(--vellum-accent-ink)]"
                          : "bg-[var(--vellum-elevated)] text-[var(--vellum-muted)]"
                      }`}
                    >
                      {ending.achieved ? <CheckCircle2 className="h-3 w-3" /> : null}
                      <span>{ending.achieved ? "本局达成" : "其他可能走向"}</span>
                    </span>
                  </div>
                  <p className="mt-1.5 leading-6 text-[var(--vellum-muted)]">
                    达成条件：{ending.conditionSummary}
                  </p>
                </article>
              ))}
            </div>
          </section>
        </div>
      ) : null}
    </main>
  );
}
