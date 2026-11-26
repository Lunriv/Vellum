import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  BookOpen,
  ChevronDown,
  ChevronUp,
  Compass,
  FileText,
  Link2,
  NotebookPen,
  Plus,
  X,
} from "lucide-react";
import { vellumApi } from "../../api/client";
import type { ClueDTO, HandoutDTO } from "../../api/types";
import { DiegeticHandoutCard } from "../dynamic_views/DiegeticHandoutCard";

interface EvidenceBoardDrawerProps {
  open: boolean;
  sessionId: string;
  fallbackClues?: ClueDTO[];
  fallbackHandouts?: HandoutDTO[];
  onClose: () => void;
}

export function EvidenceBoardDrawer({
  open,
  sessionId,
  fallbackClues = [],
  fallbackHandouts = [],
  onClose,
}: EvidenceBoardDrawerProps) {
  const queryClient = useQueryClient();

  const [expandedHandoutIds, setExpandedHandoutIds] = useState<Record<string, boolean>>({});
  const [noteTitle, setNoteTitle] = useState("");
  const [noteContent, setNoteContent] = useState("");
  const [fromClueId, setFromClueId] = useState("");
  const [toClueId, setToClueId] = useState("");
  const [linkLabel, setLinkLabel] = useState("");

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open, onClose]);

  const evidenceQuery = useQuery({
    queryKey: ["evidence", sessionId],
    queryFn: () => vellumApi.getEvidence(sessionId),
    enabled: open && Boolean(sessionId),
  });

  const createNoteMutation = useMutation({
    mutationFn: (payload: Parameters<typeof vellumApi.createEvidenceNote>[1]) =>
      vellumApi.createEvidenceNote(sessionId, payload),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["evidence", sessionId] });
    },
  });

  if (!open) return null;

  const clues = evidenceQuery.data?.clues ?? fallbackClues;
  const handouts = evidenceQuery.data?.handouts ?? fallbackHandouts;
  const notes = evidenceQuery.data?.notes ?? [];

  const personalNotes = notes.filter((n) => n.noteKind === "note");
  const clueLinks = notes.filter((n) => n.noteKind === "link");

  const clueTitleById = new Map(clues.map((c) => [c.clueId, c.title]));

  const effectiveFromId = fromClueId || clues[0]?.clueId || "";
  const effectiveToId =
    toClueId && toClueId !== effectiveFromId
      ? toClueId
      : clues.find((c) => c.clueId !== effectiveFromId)?.clueId || "";

  const toggleHandout = (handoutId: string) => {
    setExpandedHandoutIds((prev) => ({ ...prev, [handoutId]: !prev[handoutId] }));
  };

  return (
    <div
      className="fixed inset-0 z-40 flex justify-end"
      role="dialog"
      aria-modal="true"
      aria-label="线索板"
    >
      <div className="fixed inset-0 bg-black/45" aria-hidden="true" onClick={onClose} />

      <div className="relative z-10 flex h-full w-full max-w-xl flex-col border-l border-[var(--vellum-border)] bg-[var(--vellum-surface)] shadow-[var(--vellum-shadow-md)]">
        <header className="flex items-center justify-between border-b border-[var(--vellum-border)] px-5 py-3.5">
          <div>
            <h2 className="inline-flex items-center gap-2 text-sm font-semibold text-[var(--vellum-ink)]">
              <BookOpen className="h-4 w-4 text-[var(--vellum-accent)]" />
              <span>调查线索板</span>
            </h2>
            <p className="mt-0.5 text-xs text-[var(--vellum-muted)]">
              汇总已证实线索、实体物证与你的个人推理整理
            </p>
          </div>
          <button
            type="button"
            aria-label="关闭线索板"
            onClick={onClose}
            className="rounded-[var(--vellum-radius-sm)] p-1 text-[var(--vellum-muted)] hover:bg-[var(--vellum-elevated)] hover:text-[var(--vellum-ink)]"
          >
            <X className="h-4 w-4" />
          </button>
        </header>

        <div className="flex-1 space-y-6 overflow-y-auto p-5 text-xs">
          <section aria-labelledby="canonical-evidence-heading">
            <div className="flex items-baseline justify-between gap-2 border-b border-[var(--vellum-border)] pb-2">
              <h3
                id="canonical-evidence-heading"
                className="inline-flex items-center gap-1.5 text-sm font-semibold text-[var(--vellum-ink)]"
              >
                <Compass className="h-4 w-4 text-[var(--vellum-accent)]" />
                <span>已证实线索（{clues.length}）</span>
              </h3>
              <span className="text-[11px] text-[var(--vellum-muted)]">调查中核实的客观记录</span>
            </div>

            {clues.length === 0 ? (
              <p className="mt-3 text-[var(--vellum-muted)]">
                目前尚未记录到已证实线索。随着现场勘查与交谈推进，核实后的线索会自动归档于此。
              </p>
            ) : (
              <div className="mt-3 space-y-2.5">
                {clues.map((clue) => (
                  <article
                    key={clue.clueId}
                    className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-bg)]/80 p-3.5"
                  >
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <h4 className="text-xs font-semibold text-[var(--vellum-ink)]">
                        {clue.title}
                      </h4>
                      <span
                        className={`rounded px-1.5 py-0.5 text-[10px] font-medium ${
                          clue.isConclusion
                            ? "bg-[var(--vellum-accent-soft)] text-[var(--vellum-accent)]"
                            : "bg-[var(--vellum-elevated)] text-[var(--vellum-muted)]"
                        }`}
                      >
                        {clue.isConclusion ? "核心推论" : "已证实线索"}
                      </span>
                    </div>
                    <p className="mt-1.5 leading-5 text-[var(--vellum-ink)]">{clue.summary}</p>
                    <p className="mt-1.5 text-[11px] text-[var(--vellum-muted)]">
                      来源：{clue.sourceLabel}
                    </p>
                  </article>
                ))}
              </div>
            )}
          </section>

          <section aria-labelledby="handouts-heading">
            <div className="flex items-baseline justify-between gap-2 border-b border-[var(--vellum-border)] pb-2">
              <h3
                id="handouts-heading"
                className="inline-flex items-center gap-1.5 text-sm font-semibold text-[var(--vellum-ink)]"
              >
                <FileText className="h-4 w-4 text-[var(--vellum-accent)]" />
                <span>实体物证（{handouts.length}）</span>
              </h3>
              <span className="text-[11px] text-[var(--vellum-muted)]">可随时展开重读原文</span>
            </div>

            {handouts.length === 0 ? (
              <p className="mt-3 text-[var(--vellum-muted)]">尚未取得信件、剪报或手记等实体物证。</p>
            ) : (
              <div className="mt-3 space-y-2.5">
                {handouts.map((handout) => {
                  const expanded = Boolean(expandedHandoutIds[handout.handoutId]);
                  return (
                    <div
                      key={handout.handoutId}
                      className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-bg)]/60 p-3"
                    >
                      <div className="flex items-center justify-between gap-2">
                        <span className="font-semibold text-[var(--vellum-ink)]">
                          {handout.title}
                        </span>
                        <button
                          type="button"
                          onClick={() => toggleHandout(handout.handoutId)}
                          className="inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] px-2 py-1 text-[11px] font-medium text-[var(--vellum-ink)] hover:bg-[var(--vellum-elevated)]"
                        >
                          <span>{expanded ? "收起原文" : "展开阅读"}</span>
                          {expanded ? (
                            <ChevronUp className="h-3 w-3" />
                          ) : (
                            <ChevronDown className="h-3 w-3" />
                          )}
                        </button>
                      </div>
                      {expanded ? (
                        <div className="mt-3">
                          <DiegeticHandoutCard payload={handout} alreadyDisplayed />
                        </div>
                      ) : null}
                    </div>
                  );
                })}
              </div>
            )}
          </section>

          <section aria-labelledby="player-notes-heading">
            <div className="border-b border-[var(--vellum-border)] pb-2">
              <h3
                id="player-notes-heading"
                className="inline-flex items-center gap-1.5 text-sm font-semibold text-[var(--vellum-ink)]"
              >
                <NotebookPen className="h-4 w-4 text-[var(--vellum-accent)]" />
                <span>调查员个人笔记</span>
              </h3>
              <p className="mt-0.5 text-[11px] text-[var(--vellum-muted)]">
                仅作推理整理，不改变世界事实
              </p>
            </div>

            {clueLinks.length > 0 ? (
              <div className="mt-3 space-y-1.5">
                <div className="text-[11px] font-semibold text-[var(--vellum-muted)]">
                  已建立的线索关联
                </div>
                {clueLinks.map((link) => (
                  <div
                    key={link.noteId}
                    className="flex flex-wrap items-center gap-1.5 rounded-[var(--vellum-radius-sm)] border border-dashed border-[var(--vellum-border-strong)] bg-[var(--vellum-elevated)]/60 px-3 py-2 text-[var(--vellum-ink)]"
                  >
                    <Link2 className="h-3.5 w-3.5 text-[var(--vellum-accent)]" />
                    <span className="font-medium">
                      {clueTitleById.get(link.fromClueId ?? "") ?? link.fromClueId}
                    </span>
                    <span className="text-[var(--vellum-muted)]">↔</span>
                    <span className="font-medium">
                      {clueTitleById.get(link.toClueId ?? "") ?? link.toClueId}
                    </span>
                    {link.label ? (
                      <span className="text-[var(--vellum-muted)]">：{link.label}</span>
                    ) : null}
                  </div>
                ))}
              </div>
            ) : null}

            {personalNotes.length > 0 ? (
              <div className="mt-3 space-y-2">
                {personalNotes.map((note) => (
                  <article
                    key={note.noteId}
                    className="rounded-[var(--vellum-radius-sm)] border border-dashed border-[var(--vellum-border-strong)] bg-[var(--vellum-surface)] p-3"
                  >
                    {note.title ? (
                      <h4 className="font-semibold text-[var(--vellum-ink)]">{note.title}</h4>
                    ) : null}
                    {note.content ? (
                      <p className="mt-1 whitespace-pre-line leading-5 text-[var(--vellum-muted)]">
                        {note.content}
                      </p>
                    ) : null}
                  </article>
                ))}
              </div>
            ) : null}

            <form
              className="mt-4 space-y-2.5 rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-bg)]/70 p-3.5"
              onSubmit={(e) => {
                e.preventDefault();
                const title = noteTitle.trim();
                const content = noteContent.trim();
                if (!title && !content) return;
                createNoteMutation.mutate(
                  {
                    noteKind: "note",
                    title: title || "推论便签",
                    content,
                  },
                  {
                    onSuccess: () => {
                      setNoteTitle("");
                      setNoteContent("");
                    },
                  },
                );
              }}
            >
              <div className="font-semibold text-[var(--vellum-ink)]">添加个人推论便签</div>
              <input
                type="text"
                value={noteTitle}
                onChange={(e) => setNoteTitle(e.target.value)}
                placeholder="便签标题（例如：管家证词的时间矛盾）"
                className="w-full rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] px-2.5 py-1.5 text-xs text-[var(--vellum-ink)] placeholder:text-[var(--vellum-muted)] focus:border-[var(--vellum-accent)] focus:outline-none"
              />
              <textarea
                rows={2}
                value={noteContent}
                onChange={(e) => setNoteContent(e.target.value)}
                placeholder="记下你的猜想、待核实疑点或下一步查证方向……"
                className="w-full resize-y rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] px-2.5 py-1.5 text-xs text-[var(--vellum-ink)] placeholder:text-[var(--vellum-muted)] focus:border-[var(--vellum-accent)] focus:outline-none"
              />
              <button
                type="submit"
                disabled={
                  createNoteMutation.isPending || (!noteTitle.trim() && !noteContent.trim())
                }
                className="inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] bg-[var(--vellum-accent)] px-3 py-1.5 text-xs font-semibold text-[var(--vellum-accent-ink)] hover:bg-[var(--vellum-accent-hover)] disabled:opacity-50"
              >
                <Plus className="h-3.5 w-3.5" />
                <span>保存推论便签</span>
              </button>
            </form>

            <form
              className="mt-3 space-y-2.5 rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-bg)]/70 p-3.5"
              onSubmit={(e) => {
                e.preventDefault();
                if (!effectiveFromId || !effectiveToId || effectiveFromId === effectiveToId) return;
                const label = linkLabel.trim();
                if (!label) return;
                createNoteMutation.mutate(
                  {
                    noteKind: "link",
                    fromClueId: effectiveFromId,
                    toClueId: effectiveToId,
                    label,
                  },
                  {
                    onSuccess: () => {
                      setLinkLabel("");
                    },
                  },
                );
              }}
            >
              <div className="font-semibold text-[var(--vellum-ink)]">手动关联两条线索</div>
              {clues.length < 2 ? (
                <p className="text-[11px] text-[var(--vellum-muted)]">
                  至少发现两条已证实线索后，可在此建立它们之间的关联说明。
                </p>
              ) : (
                <>
                  <div className="grid gap-2 sm:grid-cols-2">
                    <select
                      aria-label="起点线索"
                      value={effectiveFromId}
                      onChange={(e) => setFromClueId(e.target.value)}
                      className="rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] px-2.5 py-1.5 text-xs text-[var(--vellum-ink)] focus:border-[var(--vellum-accent)] focus:outline-none"
                    >
                      {clues.map((c) => (
                        <option key={c.clueId} value={c.clueId}>
                          {c.title}
                        </option>
                      ))}
                    </select>
                    <select
                      aria-label="终点线索"
                      value={effectiveToId}
                      onChange={(e) => setToClueId(e.target.value)}
                      className="rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] px-2.5 py-1.5 text-xs text-[var(--vellum-ink)] focus:border-[var(--vellum-accent)] focus:outline-none"
                    >
                      {clues
                        .filter((c) => c.clueId !== effectiveFromId)
                        .map((c) => (
                          <option key={c.clueId} value={c.clueId}>
                            {c.title}
                          </option>
                        ))}
                    </select>
                  </div>
                  <div className="flex flex-col gap-2 sm:flex-row">
                    <input
                      type="text"
                      value={linkLabel}
                      onChange={(e) => setLinkLabel(e.target.value)}
                      placeholder="填写关联说明（例如：两处泥印来自同一双猎靴）"
                      className="flex-1 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] px-2.5 py-1.5 text-xs text-[var(--vellum-ink)] placeholder:text-[var(--vellum-muted)] focus:border-[var(--vellum-accent)] focus:outline-none"
                    />
                    <button
                      type="submit"
                      disabled={
                        createNoteMutation.isPending ||
                        !linkLabel.trim() ||
                        effectiveFromId === effectiveToId
                      }
                      className="inline-flex items-center justify-center gap-1 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border-strong)] bg-[var(--vellum-surface)] px-3 py-1.5 text-xs font-medium text-[var(--vellum-ink)] hover:bg-[var(--vellum-elevated)] disabled:opacity-50"
                    >
                      <Link2 className="h-3.5 w-3.5" />
                      <span>建立关联</span>
                    </button>
                  </div>
                </>
              )}
            </form>
          </section>
        </div>
      </div>
    </div>
  );
}
