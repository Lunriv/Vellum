import { useEffect, useRef, useState } from "react";
import { AlertCircle, FastForward, RotateCcw, UserRound } from "lucide-react";
import type {
  ExperienceMode,
  PendingInterruptDTO,
  ResolveInterruptRequest,
  TranscriptItemDTO,
} from "../../api/types";
import type { ActiveTurnViewState } from "../../stores/gameStore";
import { useGameStore } from "../../stores/gameStore";
import { InlineDiceCard } from "../dice/InlineDiceCard";
import { DvpDirectiveView } from "../dynamic_views/DvpRegistry";
import { TheaterTranscriptBlock } from "./TheaterTranscriptBlock";

interface NarrativeStageProps {
  transcript: TranscriptItemDTO[];
  activeTurn: ActiveTurnViewState | null;
  pendingInterrupt: PendingInterruptDTO | null;
  experienceMode: ExperienceMode;
  isResolvingInterrupt: boolean;
  interruptError?: string | null;
  isRetryingTurn?: boolean;
  onResolveInterrupt: (payload: ResolveInterruptRequest) => void;
  onRetryInterruptedAction: (actionInput: string) => void;
  onDiscardInterruptedTurn: () => void;
}

function prefersReducedMotion(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return false;
  }
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

interface StreamingNarrationBlockProps {
  narrationText: string;
  narrationComplete: boolean;
  typewriterSkipped: boolean;
  onSkipTypewriter: () => void;
}

function StreamingNarrationBlock({
  narrationText,
  narrationComplete,
  typewriterSkipped,
  onSkipTypewriter,
}: StreamingNarrationBlockProps) {
  const reducedMotion = prefersReducedMotion();
  const bypassAnimation = narrationComplete || typewriterSkipped || reducedMotion;

  const [revealedLength, setRevealedLength] = useState<number>(() =>
    bypassAnimation ? narrationText.length : Math.min(narrationText.length, 16),
  );

  useEffect(() => {
    if (bypassAnimation || revealedLength >= narrationText.length) {
      return;
    }
    const timer = window.setTimeout(() => {
      setRevealedLength((prev) => {
        const remaining = narrationText.length - prev;
        if (remaining <= 0) return narrationText.length;
        const step = Math.max(5, Math.ceil(remaining / 7));
        return Math.min(narrationText.length, prev + step);
      });
    }, 18);

    return () => window.clearTimeout(timer);
  }, [bypassAnimation, revealedLength, narrationText.length]);

  const visibleText = bypassAnimation
    ? narrationText
    : narrationText.slice(0, Math.max(revealedLength, Math.min(narrationText.length, 16)));

  const showSkipButton =
    !narrationComplete && !typewriterSkipped && !reducedMotion && narrationText.length > 0;

  if (!visibleText && !showSkipButton) {
    return null;
  }

  return (
    <div className="space-y-2.5">
      {visibleText ? (
        <TheaterTranscriptBlock text={visibleText} isStreaming={!narrationComplete} />
      ) : null}

      {showSkipButton ? (
        <div className="flex justify-end">
          <button
            type="button"
            onClick={onSkipTypewriter}
            className="inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] px-2.5 py-1 text-xs text-[var(--vellum-muted)] transition-colors hover:border-[var(--vellum-border-strong)] hover:text-[var(--vellum-ink)]"
          >
            <FastForward className="h-3 w-3" />
            <span>跳过打字机</span>
          </button>
        </div>
      ) : null}
    </div>
  );
}

export function NarrativeStage({
  transcript,
  activeTurn,
  pendingInterrupt,
  experienceMode,
  isResolvingInterrupt,
  interruptError,
  isRetryingTurn = false,
  onResolveInterrupt,
  onRetryInterruptedAction,
  onDiscardInterruptedTurn,
}: NarrativeStageProps) {
  const skipTypewriter = useGameStore((s) => s.skipTypewriter);
  const stageBottomRef = useRef<HTMLDivElement | null>(null);
  const initialRenderRef = useRef(true);

  const isActiveTurnCommittedInTranscript = activeTurn
    ? transcript.some((item) => item.turnId === activeTurn.turnId)
    : false;

  const showActiveTurn = Boolean(activeTurn && !isActiveTurnCommittedInTranscript);

  useEffect(() => {
    if (initialRenderRef.current) {
      initialRenderRef.current = false;
      return;
    }
    const el = stageBottomRef.current;
    if (el && typeof el.scrollIntoView === "function") {
      el.scrollIntoView({
        behavior: prefersReducedMotion() ? "auto" : "smooth",
        block: "end",
      });
    }
  }, [
    transcript.length,
    activeTurn?.turnId,
    activeTurn?.narrationComplete,
    pendingInterrupt?.interruptId,
    pendingInterrupt?.status,
  ]);

  return (
    <section
      aria-label="调查叙事记录"
      className="mx-auto w-full max-w-[74ch] space-y-7 pb-8 pt-6"
    >
      {transcript.length === 0 && !showActiveTurn && !pendingInterrupt ? (
        <p className="vellum-prose py-8 text-sm text-[var(--vellum-muted)]">
          雨声正穿过庄园长廊，等待守秘人翻开第一页记录……
        </p>
      ) : null}

      {transcript.map((entry) => (
        <article
          key={entry.entryId}
          aria-label={`第 ${entry.turnIndex} 回合记录`}
          className="space-y-4 border-b border-[var(--vellum-border)]/70 pb-7 last:border-b-0"
        >
          {entry.playerInput ? (
            <div className="flex justify-end">
              <blockquote className="inline-flex max-w-[88%] items-start gap-2 rounded-[var(--vellum-radius-md)] border border-[var(--vellum-wood-border)]/70 bg-[var(--vellum-elevated)] px-3.5 py-2 text-sm leading-6 text-[var(--vellum-ink)] shadow-xs">
                <UserRound className="mt-1 h-3.5 w-3.5 shrink-0 text-[var(--vellum-brass)]" />
                <span>{entry.playerInput}</span>
              </blockquote>
            </div>
          ) : null}

          {entry.narration ? <TheaterTranscriptBlock text={entry.narration} /> : null}

          {entry.directives.length > 0 ? (
            <div className="space-y-3 pt-1">
              {entry.directives.map((directive) => (
                <DvpDirectiveView
                  key={directive.directiveId}
                  directive={{ ...directive, displayed: true }}
                  experienceMode={experienceMode}
                />
              ))}
            </div>
          ) : null}
        </article>
      ))}

      {showActiveTurn && activeTurn ? (
        <article aria-label="当前进行回合" className="space-y-4">
          {activeTurn.playerInput ? (
            <div className="flex justify-end">
              <blockquote className="inline-flex max-w-[88%] items-start gap-2 rounded-[var(--vellum-radius-md)] border border-[var(--vellum-accent)]/60 bg-[var(--vellum-accent-soft)] px-3.5 py-2 text-sm leading-6 text-[var(--vellum-ink)] shadow-xs">
                <UserRound className="mt-1 h-3.5 w-3.5 shrink-0 text-[var(--vellum-accent)]" />
                <span>{activeTurn.playerInput}</span>
              </blockquote>
            </div>
          ) : null}

          {activeTurn.immediateDirectives.length > 0 ? (
            <div className="space-y-3">
              {activeTurn.immediateDirectives.map((directive) => (
                <DvpDirectiveView
                  key={directive.directiveId}
                  directive={directive}
                  experienceMode={experienceMode}
                />
              ))}
            </div>
          ) : null}

          {activeTurn.statusHint ? (
            <div
              role="status"
              aria-live="polite"
              className="inline-flex items-center gap-2 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-wood-border)] bg-[var(--vellum-surface)] px-3 py-1.5 text-xs text-[var(--vellum-brass)]"
            >
              <span
                className="h-2 w-2 animate-pulse rounded-full bg-[var(--vellum-accent)]"
                aria-hidden="true"
              />
              <span>{activeTurn.statusHint}</span>
            </div>
          ) : null}

          <StreamingNarrationBlock
            key={`${activeTurn.turnId}:${activeTurn.generationId}`}
            narrationText={activeTurn.narrationText}
            narrationComplete={activeTurn.narrationComplete}
            typewriterSkipped={activeTurn.typewriterSkipped}
            onSkipTypewriter={skipTypewriter}
          />

          {activeTurn.narrationComplete && activeTurn.releasedAfterNarration.length > 0 ? (
            <div className="space-y-3 pt-1">
              {activeTurn.releasedAfterNarration.map((directive) => (
                <DvpDirectiveView
                  key={directive.directiveId}
                  directive={directive}
                  experienceMode={experienceMode}
                />
              ))}
            </div>
          ) : null}

          {activeTurn.status === "interrupted_before_commit" ? (
            <div
              role="status"
              className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-warning)]/40 bg-[var(--vellum-warning-soft)] p-4 text-xs text-[var(--vellum-ink)]"
            >
              <div className="font-semibold">本次行动尚未结算，未产生任何检定或消耗</div>
              <p className="mt-1 leading-6 text-[var(--vellum-muted)]">
                上一次连接或服务中断发生在规则裁定之前，世界状态与角色数值均未改动。你可以直接重新提交本次行动。
              </p>
              <div className="mt-3 flex flex-wrap items-center gap-2">
                {activeTurn.playerInput ? (
                  <button
                    type="button"
                    disabled={isRetryingTurn}
                    onClick={() => onRetryInterruptedAction(activeTurn.playerInput ?? "")}
                    className="inline-flex items-center gap-1.5 rounded-[var(--vellum-radius-sm)] bg-[var(--vellum-accent)] px-3 py-1.5 text-xs font-semibold text-[var(--vellum-accent-ink)] hover:bg-[var(--vellum-accent-hover)] disabled:opacity-50"
                  >
                    <RotateCcw className="h-3.5 w-3.5" />
                    <span>{isRetryingTurn ? "正在重新提交…" : "重新提交本次行动"}</span>
                  </button>
                ) : null}
                <button
                  type="button"
                  disabled={isRetryingTurn}
                  onClick={onDiscardInterruptedTurn}
                  className="rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border-strong)] bg-[var(--vellum-surface)] px-3 py-1.5 text-xs text-[var(--vellum-muted)] hover:text-[var(--vellum-ink)] disabled:opacity-50"
                >
                  放弃并重新输入
                </button>
              </div>
            </div>
          ) : null}

          {activeTurn.status === "failed" ? (
            <div
              role="alert"
              className="flex items-start gap-2.5 rounded-[var(--vellum-radius-md)] border border-[var(--vellum-danger)]/40 bg-[var(--vellum-danger-soft)] p-4 text-xs text-[var(--vellum-ink)]"
            >
              <AlertCircle className="mt-0.5 h-4 w-4 shrink-0 text-[var(--vellum-danger)]" />
              <div className="flex-1">
                <div className="font-semibold">本回合未能完成回应</div>
                <p className="mt-0.5 text-[var(--vellum-muted)]">
                  {activeTurn.errorMessage ?? "服务响应中断，请稍后重试。"}
                </p>
                {activeTurn.playerInput ? (
                  <button
                    type="button"
                    disabled={isRetryingTurn}
                    onClick={() => onRetryInterruptedAction(activeTurn.playerInput ?? "")}
                    className="mt-2.5 inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border-strong)] bg-[var(--vellum-surface)] px-2.5 py-1 text-xs font-medium text-[var(--vellum-ink)] hover:bg-[var(--vellum-elevated)] disabled:opacity-50"
                  >
                    <RotateCcw className="h-3 w-3" />
                    <span>重试本次行动</span>
                  </button>
                ) : null}
              </div>
            </div>
          ) : null}
        </article>
      ) : null}

      {pendingInterrupt ? (
        <div className="space-y-2 pt-2">
          <InlineDiceCard
            interrupt={pendingInterrupt}
            experienceMode={experienceMode}
            busy={isResolvingInterrupt}
            onResolve={onResolveInterrupt}
          />
          {interruptError ? (
            <p role="alert" className="text-xs text-[var(--vellum-danger)]">
              {interruptError}
            </p>
          ) : null}
        </div>
      ) : null}

      <div ref={stageBottomRef} aria-hidden="true" className="h-1" />
    </section>
  );
}
