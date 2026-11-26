import { useEffect, useRef, useState } from "react";
import { HelpCircle, Send, X } from "lucide-react";
import { vellumApi } from "../../api/client";
import type { OocMessageDTO } from "../../api/types";

interface OocDrawerProps {
  open: boolean;
  sessionId: string;
  hasPendingInterrupt?: boolean;
  onClose: () => void;
}

const QUICK_QUESTIONS = [
  "孤注一掷有什么风险？",
  "我目前还剩多少幸运和理智？",
  "困难检定与极难检定怎么判定成功？",
];

export function OocDrawer({
  open,
  sessionId,
  hasPendingInterrupt = false,
  onClose,
}: OocDrawerProps) {
  const [messagesBySession, setMessagesBySession] = useState<Record<string, OocMessageDTO[]>>({});
  const [questionInput, setQuestionInput] = useState("");
  const [sending, setSending] = useState(false);
  const [errorText, setErrorText] = useState<string | null>(null);
  const sendingRef = useRef(false);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open, onClose]);

  if (!open) return null;

  const messages = messagesBySession[sessionId] ?? [];

  const submitQuestion = async (rawQuestion: string) => {
    const trimmed = rawQuestion.trim();
    if (!trimmed || !sessionId || sendingRef.current) return;

    sendingRef.current = true;
    setSending(true);
    setErrorText(null);

    try {
      const res = await vellumApi.askOoc(sessionId, { question: trimmed });
      setQuestionInput("");
      setMessagesBySession((prev) => {
        if (Array.isArray(res.history) && res.history.length > 0) {
          return { ...prev, [sessionId]: res.history };
        }
        const now = new Date().toISOString();
        const existing = prev[sessionId] ?? [];
        return {
          ...prev,
          [sessionId]: [
            ...existing,
            { id: `local-q-${Date.now()}`, role: "player", content: trimmed, createdAt: now },
            { id: `local-a-${Date.now()}`, role: "assistant", content: res.answer, createdAt: now },
          ],
        };
      });
    } catch (err) {
      setErrorText(
        err instanceof Error && err.message ? err.message : "规则问答暂时未能返回，请稍后重试。",
      );
    } finally {
      sendingRef.current = false;
      setSending(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-40 flex justify-end"
      role="dialog"
      aria-modal="true"
      aria-label="OOC 场外规则助手"
    >
      <div className="fixed inset-0 bg-black/45" aria-hidden="true" onClick={onClose} />

      <div className="relative z-10 flex h-full w-full max-w-md flex-col border-l border-[var(--vellum-border)] bg-[var(--vellum-surface)] shadow-[var(--vellum-shadow-md)]">
        <header className="flex items-start justify-between gap-3 border-b border-[var(--vellum-border)] px-5 py-3.5">
          <div>
            <h2 className="inline-flex items-center gap-1.5 text-sm font-semibold text-[var(--vellum-ink)]">
              <HelpCircle className="h-4 w-4 text-[var(--vellum-accent)]" />
              <span>OOC 规则助手</span>
            </h2>
            <p className="mt-0.5 text-xs font-medium text-[var(--vellum-accent)]">
              场外规则与状态问答 · 不推进游戏时间
            </p>
          </div>
          <button
            type="button"
            aria-label="关闭 OOC 规则助手"
            onClick={onClose}
            className="rounded-[var(--vellum-radius-sm)] p-1 text-[var(--vellum-muted)] hover:bg-[var(--vellum-elevated)] hover:text-[var(--vellum-ink)]"
          >
            <X className="h-4 w-4" />
          </button>
        </header>

        <div className="flex-1 space-y-4 overflow-y-auto p-5 text-xs">
          {hasPendingInterrupt ? (
            <div className="rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border-strong)] bg-[var(--vellum-elevated)] px-3 py-2 text-[var(--vellum-ink)]">
              当前主舞台正在等待你的检定决策；在此询问规则不会消耗回合、不会投掷骰子，也不会改动角色状态。
            </div>
          ) : null}

          <div className="space-y-1.5">
            <div className="text-[11px] font-semibold text-[var(--vellum-muted)]">常见规则速查</div>
            <div className="flex flex-wrap gap-1.5">
              {QUICK_QUESTIONS.map((q) => (
                <button
                  key={q}
                  type="button"
                  disabled={sending}
                  onClick={() => void submitQuestion(q)}
                  className="rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-bg)] px-2.5 py-1.5 text-left text-xs text-[var(--vellum-ink)] transition-colors hover:border-[var(--vellum-accent)] disabled:opacity-50"
                >
                  {q}
                </button>
              ))}
            </div>
          </div>

          {messages.length === 0 ? (
            <p className="py-6 text-center leading-6 text-[var(--vellum-muted)]">
              对检定成功等级、消耗幸运、孤注一掷或角色当前状态有疑问时，可点选上方问题或直接在下方提问。
            </p>
          ) : (
            <div className="space-y-3 pt-2">
              {messages.map((msg) => (
                <div
                  key={msg.id}
                  className={`rounded-[var(--vellum-radius-md)] border p-3 leading-6 ${
                    msg.role === "player"
                      ? "ml-6 border-[var(--vellum-border)] bg-[var(--vellum-elevated)] text-[var(--vellum-ink)]"
                      : "mr-4 border-[var(--vellum-border-strong)] bg-[var(--vellum-bg)] text-[var(--vellum-ink)]"
                  }`}
                >
                  <div className="mb-1 text-[10px] font-semibold text-[var(--vellum-muted)]">
                    {msg.role === "player" ? "你的提问" : "规则说明"}
                  </div>
                  <div className="whitespace-pre-line">{msg.content}</div>
                </div>
              ))}
            </div>
          )}

          {errorText ? (
            <p role="alert" className="text-xs text-[var(--vellum-danger)]">
              {errorText}
            </p>
          ) : null}
        </div>

        <form
          onSubmit={(e) => {
            e.preventDefault();
            void submitQuestion(questionInput);
          }}
          className="border-t border-[var(--vellum-border)] bg-[var(--vellum-surface)] p-4"
        >
          <label htmlFor="vellum-ooc-input" className="sr-only">
            输入场外规则或状态问题
          </label>
          <div className="flex items-center gap-2">
            <input
              id="vellum-ooc-input"
              type="text"
              value={questionInput}
              disabled={sending}
              onChange={(e) => setQuestionInput(e.target.value)}
              placeholder="询问规则机制或当前角色状态……"
              className="flex-1 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-bg)] px-3 py-2 text-xs text-[var(--vellum-ink)] placeholder:text-[var(--vellum-muted)] focus:border-[var(--vellum-accent)] focus:outline-none disabled:opacity-50"
            />
            <button
              type="submit"
              disabled={sending || !questionInput.trim()}
              className="inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] bg-[var(--vellum-accent)] px-3 py-2 text-xs font-semibold text-[var(--vellum-accent-ink)] hover:bg-[var(--vellum-accent-hover)] disabled:opacity-50"
            >
              <Send className="h-3.5 w-3.5" />
              <span>{sending ? "查询中…" : "提问"}</span>
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
