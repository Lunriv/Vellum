import { FileText } from "lucide-react";

export interface DiegeticHandoutPayload {
  handoutId: string;
  kind?: string;
  title: string;
  canonical?: boolean;
  clueId?: string;
  sourceFacts?: string[];
  presentationText: string;
}

interface DiegeticHandoutCardProps {
  payload: DiegeticHandoutPayload;
  alreadyDisplayed?: boolean;
}

const KIND_LABELS: Record<string, string> = {
  letter: "书信",
  diary: "日记残页",
  newspaper: "旧报剪页",
  telegram: "电报",
  report: "勘验记录",
  note: "手记",
};

export function DiegeticHandoutCard({ payload, alreadyDisplayed }: DiegeticHandoutCardProps) {
  const kindLabel = KIND_LABELS[payload.kind ?? "letter"] ?? "物证";
  const isCanonical = payload.canonical !== false;

  return (
    <article
      aria-label={`物证：${payload.title}`}
      className={`rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-handout-border)] bg-[var(--vellum-handout-bg)] px-5 py-4 text-[var(--vellum-handout-ink)] shadow-[var(--vellum-shadow-md)] ${
        alreadyDisplayed ? "" : "transition-opacity duration-300"
      }`}
    >
      <header className="flex flex-wrap items-center justify-between gap-2 border-b border-dashed border-[var(--vellum-handout-border)] pb-2 text-xs text-[var(--vellum-handout-muted)]">
        <span className="inline-flex items-center gap-1.5 font-medium">
          <FileText className="h-3.5 w-3.5" />
          <span>
            {kindLabel} · {payload.title}
          </span>
        </span>
        <span className="rounded border border-[var(--vellum-handout-border)] px-1.5 py-0.5 text-[11px]">
          {isCanonical ? "已归档物证" : "临时记录（非权威证据）"}
        </span>
      </header>

      <div className="vellum-prose mt-3 whitespace-pre-line text-[0.96rem] leading-8 text-[var(--vellum-handout-ink)]">
        {payload.presentationText}
      </div>

      {payload.sourceFacts && payload.sourceFacts.length > 0 ? (
        <footer className="mt-3 border-t border-[var(--vellum-handout-border)]/70 pt-2">
          <ul className="space-y-1 text-xs text-[var(--vellum-handout-muted)]">
            {payload.sourceFacts.map((fact, index) => (
              <li key={index}>• {fact}</li>
            ))}
          </ul>
        </footer>
      ) : null}
    </article>
  );
}
