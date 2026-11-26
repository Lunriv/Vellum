import { useRef, useState } from "react";
import { BookCheck, HelpCircle, Lock, Send, Sparkles, X } from "lucide-react";
import type { ExperienceMode, PendingInterruptDTO } from "../../api/types";
import type { SubmitTurnResult } from "../../hooks/useSessionSync";

interface ActionComposerProps {
  draft: string;
  onDraftChange: (value: string) => void;
  onSubmitAction: (input: string) => Promise<SubmitTurnResult>;
  experienceMode: ExperienceMode;
  guidedPrompts?: string[];
  pendingInterrupt: PendingInterruptDTO | null;
  isTurnBusy: boolean;
  sessionEnded: boolean;
  onOpenOoc: () => void;
  onOpenDebrief: () => void;
}

export function ActionComposer({
  draft,
  onDraftChange,
  onSubmitAction,
  experienceMode,
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
  const submitDisabled =
    inputDisabled || isTurnBusy || draft.trim().length === 0;

  const showGuidedPrompts =
    experienceMode === "guided" &&
    !sessionEnded &&
    !isLockedByInterrupt &&
    Array.isArray(guidedPrompts) &&
    guidedPrompts.length > 0;

  const handleSend = async () => {
    const trimmed = draft.trim();
    if (!trimmed || submittingRef.current || inputDisabled || isTurnBusy) return;

    submittingRef.current = true;
    setSubmitting(true);
    setBanner(null);

    try {
      const result = await onSubmitAction(trimmed);
      if (result.ok) {
        onDraftChange("");
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
      <footer className="sticky bottom-0 z-10 border-t border-[var(--vellum-border)] bg-[var(--vellum-surface)]/95 px-4 py-4 backdrop-blur-sm sm:px-6">
        <div className="mx-auto flex max-w-[68ch] flex-wrap items-center justify-between gap-3">
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
    <footer className="sticky bottom-0 z-10 border-t border-[var(--vellum-border)] bg-[var(--vellum-surface)]/95 px-4 py-3.5 backdrop-blur-sm sm:px-6">
      <div className="mx-auto max-w-[68ch] space-y-2.5">
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
            className="flex flex-wrap items-center justify-between gap-2 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border-strong)] bg-[var(--vellum-elevated)] px-3 py-2 text-xs text-[var(--vellum-ink)]"
          >
            <span className="inline-flex items-center gap-1.5">
              <Lock className="h-3.5 w-3.5 shrink-0 text-[var(--vellum-accent)]" />
              <span>当前需先完成上方的检定决策；等待期间仍可随时打开 OOC 规则助手提问。</span>
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

        {showGuidedPrompts ? (
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
            void handleSend();
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
                void handleSend();
              }
            }}
            placeholder={
              isLockedByInterrupt
                ? "请先在上方检定面板作出决定……"
                : "描述你的观察、行动或对在场人物说的话（Enter 发送，Shift+Enter 换行）……"
            }
            className="min-h-[52px] flex-1 resize-y rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-bg)] px-3.5 py-2 text-sm leading-6 text-[var(--vellum-ink)] placeholder:text-[var(--vellum-muted)] focus:border-[var(--vellum-accent)] focus:outline-none disabled:cursor-not-allowed disabled:opacity-60"
          />
          <button
            type="submit"
            disabled={submitDisabled}
            className="inline-flex h-[52px] shrink-0 items-center gap-1.5 rounded-[var(--vellum-radius-md)] bg-[var(--vellum-accent)] px-4 text-xs font-semibold text-[var(--vellum-accent-ink)] shadow-[var(--vellum-shadow-sm)] transition-colors hover:bg-[var(--vellum-accent-hover)] disabled:cursor-not-allowed disabled:opacity-50"
          >
            <Send className="h-3.5 w-3.5" />
            <span>{submitting ? "提交中…" : isTurnBusy ? "结算中…" : "行动"}</span>
          </button>
        </form>
      </div>
    </footer>
  );
}
