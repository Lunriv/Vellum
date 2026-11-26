import { useState } from "react";
import { Dices, RotateCcw, Sparkles, TriangleAlert } from "lucide-react";
import type {
  CheckDifficulty,
  ExperienceMode,
  PendingInterruptDTO,
  ResolveInterruptRequest,
  RollAttemptDTO,
  RollResultLevel,
} from "../../api/types";

interface InlineDiceCardProps {
  interrupt?: PendingInterruptDTO | null;
  roll?: RollAttemptDTO | null;
  experienceMode: ExperienceMode;
  busy?: boolean;
  onResolve?: (payload: ResolveInterruptRequest) => void;
}

const DIFFICULTY_LABEL: Record<CheckDifficulty, string> = {
  regular: "常规",
  hard: "困难",
  extreme: "极难",
};

const RESULT_LABEL: Record<RollResultLevel, string> = {
  critical_success: "大成功",
  extreme_success: "极难成功",
  hard_success: "困难成功",
  regular_success: "成功",
  failure: "失败",
  fumble: "大失败",
};

function computeThreshold(target: number, difficulty: CheckDifficulty): number {
  if (difficulty === "extreme") return Math.floor(target / 5);
  if (difficulty === "hard") return Math.floor(target / 2);
  return target;
}

export function InlineDiceCard({
  interrupt,
  roll,
  experienceMode,
  busy = false,
  onResolve,
}: InlineDiceCardProps) {
  const [reviseOpen, setReviseOpen] = useState(false);
  const [replacementAction, setReplacementAction] = useState("");
  const [pushDescription, setPushDescription] = useState("");

  const activeRoll = interrupt?.firstRoll ?? roll ?? null;
  const skillLabel = interrupt?.skillLabel ?? activeRoll?.skillLabel ?? activeRoll?.skill ?? "技能检定";
  const difficulty = interrupt?.difficulty ?? activeRoll?.difficulty ?? "regular";
  const targetNumber = interrupt?.targetNumber ?? activeRoll?.targetNumber ?? 50;
  const threshold = computeThreshold(targetNumber, difficulty);
  const bonusPenaltyDice = interrupt?.bonusPenaltyDice ?? activeRoll?.bonusPenaltyDice ?? 0;

  return (
    <section
      aria-label="技能检定"
      className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-dice-border)] bg-[var(--vellum-dice-bg)] px-4 py-3.5 text-[var(--vellum-dice-ink)] shadow-[var(--vellum-shadow-sm)]"
    >
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <div className="vellum-mono flex h-12 min-w-12 items-center justify-center rounded-[var(--vellum-radius-sm)] border border-white/15 bg-white/5 px-2.5 text-xl font-bold">
            {activeRoll ? activeRoll.rollValue : <Dices className="h-5 w-5 opacity-80" />}
          </div>
          <div>
            <div className="flex flex-wrap items-center gap-2 text-sm font-semibold">
              <span>
                {skillLabel}（{DIFFICULTY_LABEL[difficulty]}）
              </span>
              {activeRoll ? (
                <span
                  className={`rounded px-1.5 py-0.5 text-[11px] font-semibold ${
                    activeRoll.passed
                      ? "bg-[var(--vellum-success-soft)] text-[var(--vellum-success)]"
                      : "bg-[var(--vellum-danger-soft)] text-[var(--vellum-danger)]"
                  }`}
                >
                  {RESULT_LABEL[activeRoll.resultLevel]}
                </span>
              ) : (
                <span className="rounded bg-white/10 px-1.5 py-0.5 text-[11px] font-medium text-[var(--vellum-dice-muted)]">
                  等待确认掷骰
                </span>
              )}
              {activeRoll && activeRoll.pushCount > 0 ? (
                <span className="rounded bg-[var(--vellum-warning-soft)] px-1.5 py-0.5 text-[11px] text-[var(--vellum-warning)]">
                  孤注一掷终判
                </span>
              ) : null}
            </div>
            <p className="mt-0.5 text-xs text-[var(--vellum-dice-muted)]">
              目标值 {targetNumber}（{DIFFICULTY_LABEL[difficulty]} ≤ {threshold}）
              {bonusPenaltyDice > 0 ? ` · 奖励骰 +${bonusPenaltyDice}` : ""}
              {bonusPenaltyDice < 0 ? ` · 惩罚骰 ${bonusPenaltyDice}` : ""}
              {interrupt?.status === "awaiting_recovery" ? " · 首次结果已锁定，不可取消重掷" : ""}
            </p>
          </div>
        </div>

        {interrupt && onResolve && interrupt.status === "awaiting_roll" ? (
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              disabled={busy}
              onClick={() => onResolve({ action: "confirm_roll" })}
              className="rounded-[var(--vellum-radius-sm)] bg-[var(--vellum-accent)] px-3 py-1.5 text-xs font-semibold text-[var(--vellum-accent-ink)] hover:bg-[var(--vellum-accent-hover)] disabled:opacity-50"
            >
              确认掷骰
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => setReviseOpen((v) => !v)}
              className="rounded-[var(--vellum-radius-sm)] border border-white/20 px-2.5 py-1.5 text-xs hover:bg-white/10 disabled:opacity-50"
            >
              修改行动
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => onResolve({ action: "cancel" })}
              className="rounded-[var(--vellum-radius-sm)] border border-white/20 px-2.5 py-1.5 text-xs text-[var(--vellum-dice-muted)] hover:bg-white/10 hover:text-[var(--vellum-dice-ink)] disabled:opacity-50"
            >
              撤回
            </button>
          </div>
        ) : null}

        {interrupt && onResolve && interrupt.status === "awaiting_recovery" ? (
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              disabled={busy}
              onClick={() => onResolve({ action: "accept_failure" })}
              className="rounded-[var(--vellum-radius-sm)] border border-white/20 px-2.5 py-1.5 text-xs hover:bg-white/10 disabled:opacity-50"
            >
              接受失败
            </button>
            {interrupt.recovery?.canSpendLuck ? (
              <button
                type="button"
                disabled={busy}
                onClick={() => onResolve({ action: "spend_luck" })}
                className="inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] border border-white/25 bg-white/10 px-2.5 py-1.5 text-xs font-medium hover:bg-white/15 disabled:opacity-50"
              >
                <Sparkles className="h-3.5 w-3.5" />
                <span>消耗 {interrupt.recovery.luckCostNeeded} 点幸运改判</span>
              </button>
            ) : null}
            {interrupt.recovery?.canPushRoll ? (
              <button
                type="button"
                disabled={busy}
                onClick={() => onResolve({ action: "push_roll" })}
                className="inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] bg-[var(--vellum-accent)] px-3 py-1.5 text-xs font-semibold text-[var(--vellum-accent-ink)] hover:bg-[var(--vellum-accent-hover)] disabled:opacity-50"
              >
                <RotateCcw className="h-3.5 w-3.5" />
                <span>孤注一掷</span>
              </button>
            ) : null}
          </div>
        ) : null}
      </div>

      {interrupt?.stakesHint && experienceMode !== "immersive" ? (
        <p className="mt-2.5 border-t border-white/10 pt-2 text-xs text-[var(--vellum-dice-muted)]">
          风险提示：{interrupt.stakesHint}
        </p>
      ) : null}

      {experienceMode === "guided" && interrupt?.status === "awaiting_recovery" ? (
        <p className="mt-1.5 text-xs text-[var(--vellum-dice-muted)]">
          引导说明：失败不代表剧情卡死，你可以接受后果继续推进，或动用幸运 / 孤注一掷（需承担更高代价）。
        </p>
      ) : null}

      {reviseOpen && interrupt?.status === "awaiting_roll" && onResolve ? (
        <form
          className="mt-3 flex flex-col gap-2 border-t border-white/10 pt-3 sm:flex-row"
          onSubmit={(e) => {
            e.preventDefault();
            const trimmed = replacementAction.trim();
            if (!trimmed) return;
            onResolve({ action: "revise_action", replacementAction: trimmed });
            setReviseOpen(false);
            setReplacementAction("");
          }}
        >
          <input
            type="text"
            value={replacementAction}
            onChange={(e) => setReplacementAction(e.target.value)}
            placeholder="输入修改后的行动，将原子替换本次检定……"
            className="flex-1 rounded-[var(--vellum-radius-sm)] border border-white/20 bg-black/25 px-3 py-1.5 text-xs text-[var(--vellum-dice-ink)] placeholder:text-[var(--vellum-dice-muted)] focus:border-[var(--vellum-accent)] focus:outline-none"
          />
          <button
            type="submit"
            disabled={busy || !replacementAction.trim()}
            className="rounded-[var(--vellum-radius-sm)] bg-[var(--vellum-accent)] px-3 py-1.5 text-xs font-semibold text-[var(--vellum-accent-ink)] disabled:opacity-50"
          >
            提交替代行动
          </button>
        </form>
      ) : null}

      {interrupt?.status === "awaiting_push_description" && onResolve ? (
        <form
          className="mt-3 space-y-2 border-t border-white/10 pt-3"
          onSubmit={(e) => {
            e.preventDefault();
            const trimmed = pushDescription.trim();
            if (!trimmed) return;
            onResolve({ action: "submit_push_description", description: trimmed });
            setPushDescription("");
          }}
        >
          <label className="block text-xs font-medium text-[var(--vellum-dice-ink)]">
            请说明你准备如何换一种方式再次尝试（孤注一掷）：
          </label>
          <div className="flex flex-col gap-2 sm:flex-row">
            <input
              type="text"
              value={pushDescription}
              onChange={(e) => setPushDescription(e.target.value)}
              placeholder="例如：趁他转身倒茶时，强行压近一步盯着他的反应……"
              className="flex-1 rounded-[var(--vellum-radius-sm)] border border-white/20 bg-black/25 px-3 py-1.5 text-xs text-[var(--vellum-dice-ink)] placeholder:text-[var(--vellum-dice-muted)] focus:border-[var(--vellum-accent)] focus:outline-none"
            />
            <button
              type="submit"
              disabled={busy || !pushDescription.trim()}
              className="rounded-[var(--vellum-radius-sm)] bg-[var(--vellum-accent)] px-3 py-1.5 text-xs font-semibold text-[var(--vellum-accent-ink)] disabled:opacity-50"
            >
              提交重试方式
            </button>
          </div>
        </form>
      ) : null}

      {interrupt?.status === "awaiting_push_consequence_ack" && onResolve ? (
        <div className="mt-3 space-y-2.5 border-t border-white/10 pt-3">
          <div className="flex items-start gap-2 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-warning)]/40 bg-[var(--vellum-warning-soft)] px-3 py-2 text-xs text-[var(--vellum-dice-ink)]">
            <TriangleAlert className="mt-0.5 h-4 w-4 shrink-0 text-[var(--vellum-warning)]" />
            <div>
              <div className="font-semibold">二次失败的升级代价：</div>
              <p className="mt-0.5 opacity-90">
                {interrupt.pushConsequence ?? "再次失败将导致事态显著恶化，并无法再次补救。"}
              </p>
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              disabled={busy}
              onClick={() => onResolve({ action: "confirm_push_consequence" })}
              className="rounded-[var(--vellum-radius-sm)] bg-[var(--vellum-accent)] px-3 py-1.5 text-xs font-semibold text-[var(--vellum-accent-ink)] hover:bg-[var(--vellum-accent-hover)] disabled:opacity-50"
            >
              确认承担风险并再次掷骰
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => onResolve({ action: "accept_initial_failure" })}
              className="rounded-[var(--vellum-radius-sm)] border border-white/20 px-2.5 py-1.5 text-xs hover:bg-white/10 disabled:opacity-50"
            >
              放弃孤注一掷，接受首次失败
            </button>
          </div>
        </div>
      ) : null}
    </section>
  );
}
