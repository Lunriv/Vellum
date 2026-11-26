import { Clock, Compass, HeartPulse, Sparkles } from "lucide-react";
import type {
  DynamicViewDirective,
  ExperienceMode,
  RollAttemptDTO,
} from "../../api/types";
import { InlineDiceCard } from "../dice/InlineDiceCard";
import { DiegeticHandoutCard, type DiegeticHandoutPayload } from "./DiegeticHandoutCard";

interface DvpDirectiveViewProps {
  directive: DynamicViewDirective;
  experienceMode: ExperienceMode;
}

/**
 * Typed Component Registry for the Dynamic View Protocol (DVP).
 * Strictly whitelisted React components; never executes arbitrary HTML or JS.
 */
export function DvpDirectiveView({ directive, experienceMode }: DvpDirectiveViewProps) {
  const payload = directive.payload;

  switch (directive.directiveType) {
    case "inline_dice": {
      const roll = payload as unknown as RollAttemptDTO;
      if (typeof roll.rollValue !== "number" || typeof roll.skill !== "string") {
        return null;
      }
      return <InlineDiceCard roll={roll} experienceMode={experienceMode} />;
    }

    case "diegetic_handout": {
      const handout = payload as unknown as DiegeticHandoutPayload;
      if (typeof handout.title !== "string" || typeof handout.presentationText !== "string") {
        return null;
      }
      return <DiegeticHandoutCard payload={handout} alreadyDisplayed={directive.displayed} />;
    }

    case "evidence_unlocked": {
      const title = typeof payload.title === "string" ? payload.title : "新线索";
      const summary = typeof payload.summary === "string" ? payload.summary : "";
      const sourceLabel = typeof payload.sourceLabel === "string" ? payload.sourceLabel : "";
      return (
        <div className="flex items-start gap-2.5 rounded-[var(--vellum-radius-md)] border border-[var(--vellum-accent)]/30 bg-[var(--vellum-accent-soft)] px-4 py-3 text-sm">
          <Compass className="mt-0.5 h-4 w-4 shrink-0 text-[var(--vellum-accent)]" />
          <div>
            <div className="font-semibold text-[var(--vellum-ink)]">已记录线索：{title}</div>
            {summary ? <p className="mt-0.5 text-xs text-[var(--vellum-muted)]">{summary}</p> : null}
            {sourceLabel ? (
              <p className="mt-1 text-[11px] text-[var(--vellum-muted)]">来源：{sourceLabel}</p>
            ) : null}
          </div>
        </div>
      );
    }

    case "clock_pulse": {
      const name = typeof payload.name === "string" ? payload.name : "进度钟";
      const current = typeof payload.currentValue === "number" ? payload.currentValue : 0;
      const max = typeof payload.maxValue === "number" ? payload.maxValue : 6;
      const stageLabel = typeof payload.stageLabel === "string" ? payload.stageLabel : "";
      const reason = typeof payload.reason === "string" ? payload.reason : "";
      return (
        <div className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border-strong)] bg-[var(--vellum-elevated)] px-4 py-3 text-sm">
          <div className="flex items-center justify-between gap-2">
            <span className="inline-flex items-center gap-1.5 font-semibold text-[var(--vellum-ink)]">
              <Clock className="h-4 w-4 text-[var(--vellum-accent)]" />
              <span>{name}</span>
            </span>
            <span className="vellum-mono text-xs font-semibold text-[var(--vellum-accent)]">
              {current} / {max}
            </span>
          </div>
          <div className="mt-2 grid gap-1" style={{ gridTemplateColumns: `repeat(${Math.max(1, max)}, minmax(0, 1fr))` }}>
            {Array.from({ length: Math.max(1, max) }).map((_, i) => (
              <span
                key={i}
                className={`h-1.5 rounded-sm ${
                  i < current ? "bg-[var(--vellum-accent)]" : "bg-[var(--vellum-border)]"
                }`}
              />
            ))}
          </div>
          {stageLabel ? <p className="mt-2 text-xs font-medium text-[var(--vellum-ink)]">{stageLabel}</p> : null}
          {reason ? <p className="mt-0.5 text-xs text-[var(--vellum-muted)]">原因：{reason}</p> : null}
        </div>
      );
    }

    case "character_state_delta": {
      const deltas = Array.isArray(payload.deltas) ? payload.deltas : [];
      const reason = typeof payload.reason === "string" ? payload.reason : "";
      if (deltas.length === 0) return null;
      return (
        <div className="flex flex-wrap items-center gap-2 rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] px-3.5 py-2.5 text-xs">
          <HeartPulse className="h-3.5 w-3.5 text-[var(--vellum-accent)]" />
          <span className="font-medium text-[var(--vellum-ink)]">状态变化：</span>
          {deltas.map((item, idx) => {
            const label =
              typeof item === "object" && item !== null && "label" in item
                ? String((item as { label: unknown }).label)
                : String(item);
            return (
              <span
                key={idx}
                className="vellum-mono rounded bg-[var(--vellum-elevated)] px-2 py-0.5 text-[var(--vellum-ink)]"
              >
                {label}
              </span>
            );
          })}
          {reason ? <span className="text-[var(--vellum-muted)]">（{reason}）</span> : null}
        </div>
      );
    }

    case "atmosphere_shift": {
      const sceneTitle = typeof payload.sceneTitle === "string" ? payload.sceneTitle : "";
      const note = typeof payload.note === "string" ? payload.note : "";
      if (!sceneTitle && !note) return null;
      return (
        <div className="border-y border-[var(--vellum-border)] py-2 text-center text-xs tracking-wide text-[var(--vellum-muted)]">
          {sceneTitle ? <span className="font-medium text-[var(--vellum-ink)]">{sceneTitle}</span> : null}
          {sceneTitle && note ? " · " : ""}
          {note}
        </div>
      );
    }

    case "narrative_card": {
      const title = typeof payload.title === "string" ? payload.title : "现场侧记";
      const body = typeof payload.body === "string" ? payload.body : "";
      if (!body) return null;
      return (
        <aside className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] px-4 py-3 text-xs text-[var(--vellum-muted)]">
          <div className="mb-1 inline-flex items-center gap-1.5 font-semibold text-[var(--vellum-ink)]">
            <Sparkles className="h-3.5 w-3.5 text-[var(--vellum-accent)]" />
            <span>{title}</span>
            <span className="text-[10px] font-normal text-[var(--vellum-muted)]">（氛围侧记）</span>
          </div>
          <p className="vellum-prose text-sm leading-7 text-[var(--vellum-ink)]">{body}</p>
        </aside>
      );
    }

    default:
      return null;
  }
}
