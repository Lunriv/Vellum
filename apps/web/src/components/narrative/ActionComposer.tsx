import { useRef, useState } from "react";
import { BookCheck, ChevronRight, HelpCircle, Lock, Send, Sparkles, X } from "lucide-react";
import type { ExperienceMode, PendingInterruptDTO } from "../../api/types";
import type { SubmitTurnResult } from "../../hooks/useSessionSync";

interface ActionComposerProps {
  draft: string;
  onDraftChange: (value: string) => void;
  onSubmitAction: (input: string) => Promise<SubmitTurnResult>;
  experienceMode: ExperienceMode;
  dynamicChoices?: string[];
  guidedPrompts?: string[];
  pendingInterrupt: PendingInterruptDTO | null;
  isTurnBusy: boolean;
  sessionEnded: boolean;
  onOpenOoc: () => void;
  onOpenDebrief: () => void;
}

function splitSkillTag(choice: string): { tag: string | null; text: string } {
  const match = choice.match(/^\[([^\]]{1,12})\]\s*(.+)$/);
  if (match) {
    return { tag: match[1], text: match[2].trim() };
  }
  return { tag: null, text: choice };
}

export function ActionComposer({
  draft,
  onDraftChange,
  onSubmitAction,
  experienceMode,
  dynamicChoices,
  guidedPrompts,
  pendingInterrupt,
  isTurnBusy,
  sessionEnded,
  onOpenOoc,
  onOpenDebrief,
}: ActionComposerProps) {
  const [submitting, setSubmitting] = useState(false);
  const [banner, setBanner] = useState<{ kind: "conflict" | "error"; text: string } | null>(null);
  const submittingRef = useRef(false);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);

  const isLockedByInterrupt = Boolean(pendingInterrupt);
  const inputDisabled = sessionEnded || isLockedByInterrupt || submitting;
  const submitDisabled = inputDisabled || isTurnBusy || draft.trim().length === 0;

  const hasDynamicChoices =
    experienceMode !== "immersive" &&
    !sessionEnded &&
    !isLockedByInterrupt &&
    !isTurnBusy &&
    Array.isArray(dynamicChoices) &&
    dynamicChoices.length > 0;

  const showFallbackGuidedPrompts =
    !hasDynamicChoices &&
    experienceMode === "guided" &&
    !sessionEnded &&
    !isLockedByInterrupt &&
    Array.isArray(guidedPrompts) &&
    guidedPrompts.length > 0;

  const executeTextAction = async (rawText: string, clearDraftOnSuccess: boolean) => {
    const trimmed = rawText.trim();
    if (!trimmed || submittingRef.current || inputDisabled || isTurnBusy) return;

    submittingRef.current = true;
    setSubmitting(true);
    setBanner(null);

    try {
      const result = await onSubmitAction(trimmed);
      if (result.ok) {
        if (clearDraftOnSuccess) onDraftChange("");
      } else if (result.conflict) {
        setBanner({
          kind: "conflict",
          text:
            result.errorMessage ??
            "当前已有正在进行的行动或待决检定，已为你自动刷新同步最新状态。",
        });
      } else if (result.errorMessage) {
        setBanner({
          kind: "error",
          text: result.errorMessage,
        });
      }
    } finally {
      submittingRef.current = false;
      setSubmitting(false);
    }
  };

  if (sessionEnded) {
    return (
      <footer className="sticky bottom-0 z-10 border-t border-[var(--vellum-wood-border)]/70 bg-[var(--vellum-surface)]/95 px-4 py-4 backdrop-blur-sm sm:px-6">
        <div className="mx-auto flex max-w-[74ch] flex-wrap items-center justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-[var(--vellum-ink)]">本局调查已落下帷幕</div>
            <p className="text-xs text-[var(--vellum-muted)]">
              你可以翻阅完整结案卷宗了解幕后真相，或打开 OOC 助手回顾本局记录。
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              onClick={onOpenOoc}
              className="inline-flex items-center gap-1.5 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-elevated)] px-3 py-1.5 text-xs font-medium text-[var(--vellum-ink)] hover:border-[var(--vellum-border-strong)]"
            >
              <HelpCircle className="h-3.5 w-3.5" />
              <span>打开 OOC 助手</span>
            </button>
            <button
              type="button"
              onClick={onOpenDebrief}
              className="inline-flex items-center gap-1.5 rounded-[var(--vellum-radius-sm)] bg-[var(--vellum-accent)] px-3.5 py-1.5 text-xs font-semibold text-[var(--vellum-accent-ink)] hover:bg-[var(--vellum-accent-hover)]"
            >
              <BookCheck className="h-3.5 w-3.5" />
              <span>查看结案真相复盘</span>
            </button>
          </div>
        </div>
      </footer>
    );
  }

  return (
    <footer className="sticky bottom-0 z-10 border-t border-[var(--vellum-wood-border)]/70 bg-[var(--vellum-surface)]/95 px-4 py-3.5 backdrop-blur-sm sm:px-6">
      <div className="mx-auto max-w-[74ch] space-y-2.5">
        {banner ? (
          <div
            role={banner.kind === "conflict" ? "status" : "alert"}
            className={`flex items-center justify-between gap-2 rounded-[var(--vellum-radius-sm)] border px-3 py-2 text-xs ${
              banner.kind === "conflict"
                ? "border-[var(--vellum-warning)]/40 bg-[var(--vellum-warning-soft)] text-[var(--vellum-ink)]"
                : "border-[var(--vellum-danger)]/40 bg-[var(--vellum-danger-soft)] text-[var(--vellum-ink)]"
            }`}
          >
            <span>{banner.text}</span>
            <button
              type="button"
              aria-label="关闭提示"
              onClick={() => setBanner(null)}
              className="rounded p-0.5 text-[var(--vellum-muted)] hover:text-[var(--vellum-ink)]"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
        ) : null}

        {isLockedByInterrupt ? (
          <div
            role="status"
            className="flex flex-wrap items-center justify-between gap-2 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-wood-border)] bg-[var(--vellum-elevated)] px-3 py-2 text-xs text-[var(--vellum-ink)]"
          >
            <span className="inline-flex items-center gap-1.5">
              <Lock className="h-3.5 w-3.5 shrink-0 text-[var(--vellum-accent)]" />
              <span>当前需先在左侧呢绒骰盘或上方检定卡完成掷骰决策；等待期间仍可随时打开 OOC 提问。</span>
            </span>
            <button
              type="button"
              onClick={onOpenOoc}
              className="inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border-strong)] bg-[var(--vellum-surface)] px-2.5 py-1 text-xs font-medium text-[var(--vellum-ink)] hover:bg-[var(--vellum-bg)]"
            >
              <HelpCircle className="h-3.5 w-3.5 text-[var(--vellum-accent)]" />
              <span>打开 OOC 规则助手</span>
            </button>
          </div>
        ) : null}

        {/* Disco Elysium style 1-click contextual choices from Keeper */}
        {hasDynamicChoices ? (
          <div className="space-y-1.5" aria-label="守秘人建议行动">
            <div className="flex items-center justify-between text-[11px] font-semibold text-[var(--vellum-brass)]">
              <span className="inline-flex items-center gap-1">
                <Sparkles className="h-3 w-3" />
                <span>即刻回应（点击直接执行，或在下方自由输入）</span>
              </span>
            </div>
            <div className="grid gap-1.5">
              {dynamicChoices.slice(0, 3).map((choice, idx) => {
                const { tag, text } = splitSkillTag(choice);
                return (
                  <button
                    key={idx}
                    type="button"
                    disabled={submitting || isTurnBusy}
                    onClick={() => void executeTextAction(choice, false)}
                    className="group flex w-full items-center gap-2.5 rounded-[var(--vellum-radius-md)] border border-[var(--vellum-wood-border)]/80 bg-[var(--vellum-bg)]/90 px-3 py-2 text-left text-xs text-[var(--vellum-ink)] transition-all hover:border-[var(--vellum-accent)] hover:bg-[var(--vellum-accent-soft)] disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    <span className="vellum-mono flex h-5 w-5 shrink-0 items-center justify-center rounded bg-[var(--vellum-accent)] text-[11px] font-bold text-[var(--vellum-accent-ink)]">
                      {idx + 1}
                    </span>
                    {tag ? (
                      <span className="shrink-0 rounded border border-[var(--vellum-spk-skill)]/50 bg-[var(--vellum-spk-skill-bg)] px-1.5 py-0.5 text-[10px] font-semibold text-[var(--vellum-spk-skill)]">
                        {tag}
                      </span>
                    ) : null}
                    <span className="min-w-0 flex-1 font-medium leading-5">{text}</span>
                    <ChevronRight className="h-3.5 w-3.5 shrink-0 text-[var(--vellum-muted)] transition-transform group-hover:translate-x-0.5 group-hover:text-[var(--vellum-accent)]" />
                  </button>
                );
              })}
            </div>
          </div>
        ) : null}

        {showFallbackGuidedPrompts ? (
          <div className="flex flex-wrap items-center gap-1.5" aria-label="行动启发建议">
            <span className="inline-flex items-center gap-1 text-[11px] font-medium text-[var(--vellum-muted)]">
              <Sparkles className="h-3 w-3 text-[var(--vellum-accent)]" />
              <span>行动启发：</span>
            </span>
            {guidedPrompts.map((prompt, idx) => (
              <button
                key={idx}
                type="button"
                onClick={() => {
                  onDraftChange(prompt);
                  textareaRef.current?.focus();
                }}
                className="rounded-full border border-[var(--vellum-border)] bg-[var(--vellum-elevated)]/80 px-2.5 py-1 text-xs text-[var(--vellum-ink)] transition-colors hover:border-[var(--vellum-accent)] hover:bg-[var(--vellum-accent-soft)]/50"
              >
                {prompt}
              </button>
            ))}
          </div>
        ) : null}

        <form
          onSubmit={(e) => {
            e.preventDefault();
            void executeTextAction(draft, true);
          }}
          className="flex items-end gap-2"
        >
          <label htmlFor="vellum-action-input" className="sr-only">
            输入你的调查行动或发言
          </label>
          <textarea
            id="vellum-action-input"
            ref={textareaRef}
            rows={2}
            value={draft}
            disabled={inputDisabled}
            onChange={(e) => onDraftChange(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                e.preventDefault();
                void executeTextAction(draft, true);
              }
            }}
            placeholder={
              isLockedByInterrupt
                ? "请先在左侧骰盘或上方检定卡完成掷骰决策……"
                : "自由扮演：说出你的台词、盘问疑点，或尝试任何天马行空的破局做法（Enter 发送）……"
            }
            className="min-h-[50px] flex-1 resize-y rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-bg)] px-3.5 py-2 text-sm leading-6 text-[var(--vellum-ink)] placeholder:text-[var(--vellum-muted)] focus:border-[var(--vellum-accent)] focus:outline-none disabled:cursor-not-allowed disabled:opacity-60"
          />
          <button
            type="submit"
            disabled={submitDisabled}
            className="inline-flex h-[50px] shrink-0 items-center gap-1.5 rounded-[var(--vellum-radius-md)] bg-[var(--vellum-accent)] px-4 text-xs font-bold text-[var(--vellum-accent-ink)] shadow-[var(--vellum-shadow-sm)] transition-colors hover:bg-[var(--vellum-accent-hover)] disabled:cursor-not-allowed disabled:opacity-50"
          >
            <Send className="h-3.5 w-3.5" />
            <span>{submitting ? "提交中…" : isTurnBusy ? "对戏中…" : "行动"}</span>
          </button>
        </form>
      </div>
    </footer>
  );
}
