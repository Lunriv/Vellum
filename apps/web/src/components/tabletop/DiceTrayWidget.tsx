import { useEffect, useRef, useState } from "react";
import { Dices, RotateCcw, Sparkles } from "lucide-react";
import type {
  CheckDifficulty,
  PendingInterruptDTO,
  ResolveInterruptRequest,
  RollAttemptDTO,
  RollResultLevel,
} from "../../api/types";

interface DiceTrayWidgetProps {
  pendingInterrupt: PendingInterruptDTO | null;
  lastRoll?: RollAttemptDTO | null;
  busy?: boolean;
  onResolveInterrupt?: (payload: ResolveInterruptRequest) => void;
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

function splitD100(value: number): { tens: string; units: string } {
  if (value === 100) return { tens: "00", units: "0" };
  const clamped = Math.max(1, Math.min(99, value));
  const tensDigit = Math.floor(clamped / 10) * 10;
  const unitsDigit = clamped % 10;
  return {
    tens: tensDigit === 0 ? "00" : String(tensDigit),
    units: String(unitsDigit),
  };
}

function computeThreshold(target: number, difficulty: CheckDifficulty): number {
  if (difficulty === "extreme") return Math.floor(target / 5);
  if (difficulty === "hard") return Math.floor(target / 2);
  return target;
}

function PolyhedralD10({
  value,
  variant,
  rolling,
  label,
}: {
  value: string;
  variant: "tens" | "units";
  rolling: boolean;
  label: string;
}) {
  const fill = variant === "tens" ? "#7e2220" : "#1f475f";
  const stroke = variant === "tens" ? "#e4b25c" : "#8ec6e6";
  return (
    <div className="flex flex-col items-center gap-1">
      <div className={`relative h-14 w-14 ${rolling ? "vellum-dice-rolling" : ""}`}>
        <svg viewBox="0 0 64 64" className="h-full w-full drop-shadow-md" aria-hidden="true">
          <polygon
            points="32,3 59,24 49,58 15,58 5,24"
            fill={fill}
            stroke={stroke}
            strokeWidth="2.2"
          />
          <polygon
            points="32,3 48,27 32,48 16,27"
            fill="rgba(255,255,255,0.08)"
            stroke="rgba(255,255,255,0.18)"
            strokeWidth="1"
          />
          <line x1="32" y1="48" x2="32" y2="58" stroke="rgba(255,255,255,0.22)" strokeWidth="1" />
          <line x1="16" y1="27" x2="5" y2="24" stroke="rgba(255,255,255,0.2)" strokeWidth="1" />
          <line x1="48" y1="27" x2="59" y2="24" stroke="rgba(255,255,255,0.2)" strokeWidth="1" />
        </svg>
        <span className="vellum-mono absolute inset-0 flex items-center justify-center pt-0.5 text-lg font-extrabold tracking-tight text-[#fff8eb]">
          {value}
        </span>
      </div>
      <span className="text-[10px] tracking-wide text-[var(--vellum-dice-muted)]">{label}</span>
    </div>
  );
}

export function DiceTrayWidget({
  pendingInterrupt,
  lastRoll = null,
  busy = false,
  onResolveInterrupt,
}: DiceTrayWidgetProps) {
  const [rolling, setRolling] = useState(false);
  const [practiceRoll, setPracticeRoll] = useState<number | null>(null);
  const timerRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current);
      }
    };
  }, []);

  const authoritativeRoll = pendingInterrupt?.firstRoll ?? lastRoll;
  const displayValue = pendingInterrupt
    ? (pendingInterrupt.firstRoll?.rollValue ?? null)
    : (practiceRoll ?? lastRoll?.rollValue ?? null);
  const digits =
    displayValue !== null ? splitD100(displayValue) : { tens: "00", units: "0" };

  const triggerRollAnimation = (after?: () => void) => {
    setRolling(true);
    if (timerRef.current !== null) window.clearTimeout(timerRef.current);
    timerRef.current = window.setTimeout(() => {
      setRolling(false);
      after?.();
    }, 360);
  };

  const handleConfirmCheckRoll = () => {
    if (!onResolveInterrupt || busy) return;
    setPracticeRoll(null);
    triggerRollAnimation(() => {
      onResolveInterrupt({ action: "confirm_roll" });
    });
  };

  const handlePracticeRoll = () => {
    if (pendingInterrupt || busy) return;
    const simulated = Math.floor(Math.random() * 100) + 1;
    triggerRollAnimation(() => {
      setPracticeRoll(simulated);
    });
  };

  const threshold = pendingInterrupt
    ? computeThreshold(pendingInterrupt.targetNumber, pendingInterrupt.difficulty)
    : null;

  return (
    <section
      aria-label="实体百面骰盘"
      className={`vellum-felt-tray rounded-[var(--vellum-radius-lg)] p-3.5 transition-all ${
        pendingInterrupt ? "ring-2 ring-[var(--vellum-brass)]" : ""
      }`}
    >
      <div className="flex items-center justify-between gap-2 border-b border-white/10 pb-2">
        <div className="inline-flex items-center gap-1.5 text-xs font-semibold tracking-wide text-[#f0d18c]">
          <Dices className="h-3.5 w-3.5" />
          <span>呢绒骰盘 · d100</span>
        </div>
        {pendingInterrupt ? (
          <span className="rounded bg-[#9f2a23] px-2 py-0.5 text-[10px] font-semibold text-[#fff7ea]">
            检定待决
          </span>
        ) : practiceRoll !== null ? (
          <span className="text-[10px] text-[var(--vellum-dice-muted)]">手感试掷 · 不计入剧情</span>
        ) : authoritativeRoll ? (
          <span
            className={`rounded px-1.5 py-0.5 text-[10px] font-semibold ${
              authoritativeRoll.passed
                ? "bg-[#235d3e] text-[#dcf7e6]"
                : "bg-[#802724] text-[#fde4e2]"
            }`}
          >
            上次：{RESULT_LABEL[authoritativeRoll.resultLevel]}
          </span>
        ) : (
          <span className="text-[10px] text-[var(--vellum-dice-muted)]">内核确定性裁定</span>
        )}
      </div>

      <div className="mt-3 flex items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <PolyhedralD10 value={digits.tens} variant="tens" rolling={rolling} label="十位 d10" />
          <PolyhedralD10 value={digits.units} variant="units" rolling={rolling} label="个位 d10" />
        </div>

        <div className="min-w-0 flex-1 text-right">
          {pendingInterrupt ? (
            <>
              <div className="truncate text-xs font-semibold text-[#fff6e5]">
                {pendingInterrupt.skillLabel} · {DIFFICULTY_LABEL[pendingInterrupt.difficulty]}检定
              </div>
              <div className="vellum-mono mt-0.5 text-xs text-[#f0d18c]">
                需 ≤ {threshold}（技能 {pendingInterrupt.targetNumber}%）
              </div>
              {pendingInterrupt.firstRoll ? (
                <div className="vellum-mono mt-1 text-lg font-bold text-[#ff9b93]">
                  掷出 {pendingInterrupt.firstRoll.rollValue}
                </div>
              ) : (
                <div className="mt-1 text-[11px] text-[var(--vellum-dice-muted)]">
                  准备就绪，落骰生效
                </div>
              )}
            </>
          ) : (
            <>
              <div className="vellum-mono text-2xl font-extrabold text-[#f7e7c1]">
                {displayValue !== null ? String(displayValue).padStart(2, "0") : "—"}
              </div>
              <div className="mt-0.5 text-[11px] text-[var(--vellum-dice-muted)]">
                {practiceRoll !== null
                  ? "桌面试掷手感"
                  : authoritativeRoll
                    ? `${authoritativeRoll.skillLabel ?? authoritativeRoll.skill} · 目标 ${authoritativeRoll.targetNumber}`
                    : "待触发检定"}
              </div>
            </>
          )}
        </div>
      </div>

      {/* Live interrupt actions directly inside the felt tray */}
      {pendingInterrupt && onResolveInterrupt ? (
        <div className="mt-3 flex flex-wrap items-center justify-end gap-1.5 border-t border-white/10 pt-2.5">
          {pendingInterrupt.status === "awaiting_roll" ? (
            <>
              <button
                type="button"
                disabled={busy || rolling}
                onClick={handleConfirmCheckRoll}
                className="inline-flex flex-1 items-center justify-center gap-1.5 rounded-[var(--vellum-radius-sm)] bg-[#d8a34a] px-3 py-1.5 text-xs font-bold text-[#17120a] shadow-xs transition-colors hover:bg-[#e7b560] disabled:opacity-50"
              >
                <Dices className="h-3.5 w-3.5" />
                <span>{rolling || busy ? "掷骰中…" : "投掷百面骰"}</span>
              </button>
              <button
                type="button"
                disabled={busy || rolling}
                onClick={() => onResolveInterrupt({ action: "cancel" })}
                className="rounded-[var(--vellum-radius-sm)] border border-white/20 px-2.5 py-1.5 text-xs text-[var(--vellum-dice-muted)] hover:bg-white/10 hover:text-white disabled:opacity-50"
              >
                撤回行动
              </button>
            </>
          ) : null}

          {pendingInterrupt.status === "awaiting_recovery" ? (
            <>
              {pendingInterrupt.recovery?.canSpendLuck ? (
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => onResolveInterrupt({ action: "spend_luck" })}
                  className="inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] border border-[#e4b25c]/50 bg-[#e4b25c]/15 px-2.5 py-1.5 text-xs font-semibold text-[#f9e1aa] hover:bg-[#e4b25c]/25 disabled:opacity-50"
                >
                  <Sparkles className="h-3.5 w-3.5" />
                  <span>幸运 -{pendingInterrupt.recovery.luckCostNeeded}</span>
                </button>
              ) : null}
              {pendingInterrupt.recovery?.canPushRoll ? (
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => onResolveInterrupt({ action: "push_roll" })}
                  className="inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] bg-[#d8a34a] px-2.5 py-1.5 text-xs font-bold text-[#17120a] hover:bg-[#e7b560] disabled:opacity-50"
                >
                  <RotateCcw className="h-3.5 w-3.5" />
                  <span>孤注一掷</span>
                </button>
              ) : null}
              <button
                type="button"
                disabled={busy}
                onClick={() => onResolveInterrupt({ action: "accept_failure" })}
                className="rounded-[var(--vellum-radius-sm)] border border-white/20 px-2.5 py-1.5 text-xs text-[var(--vellum-dice-muted)] hover:bg-white/10 hover:text-white disabled:opacity-50"
              >
                接受失败
              </button>
            </>
          ) : null}

          {(pendingInterrupt.status === "awaiting_push_description" ||
            pendingInterrupt.status === "awaiting_push_consequence_ack") ? (
            <p className="w-full text-[11px] text-[#f0d18c]">
              请在右侧检定卡确认孤注一掷的方式与风险代价。
            </p>
          ) : null}
        </div>
      ) : (
        <div className="mt-2.5 flex items-center justify-between border-t border-white/10 pt-2">
          <span className="text-[11px] text-[var(--vellum-dice-muted)]">
            发生检定时可在此直接掷骰
          </span>
          <button
            type="button"
            disabled={rolling}
            onClick={handlePracticeRoll}
            className="inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] border border-white/20 bg-white/5 px-2.5 py-1 text-[11px] font-medium text-[#f4ead4] transition-colors hover:border-[#d8a34a] hover:bg-white/10 disabled:opacity-50"
          >
            <Dices className="h-3 w-3 text-[#d8a34a]" />
            <span>试掷手感</span>
          </button>
        </div>
      )}
    </section>
  );
}
