import { Brain, Compass, Flame, Sparkles } from "lucide-react";

export type TheaterBeatKind = "kp" | "npc" | "skill" | "hook" | "prose";

export interface TheaterBeat {
  kind: TheaterBeatKind;
  speaker: string;
  body: string;
}

export interface ParsedTheaterNarration {
  beats: TheaterBeat[];
  choices: string[];
}

function cleanPlainText(raw: string): string {
  return raw.replace(/\*\*/g, "").trim();
}

function npcMonogram(speaker: string): { glyph: string; tone: string } {
  if (speaker.includes("Arthur") || speaker.includes("管家")) {
    return { glyph: "管", tone: "#b6772b" };
  }
  if (speaker.includes("Evelyn") || speaker.includes("女主人")) {
    return { glyph: "伊", tone: "#9d3338" };
  }
  if (speaker.includes("Mary") || speaker.includes("女仆")) {
    return { glyph: "玛", tone: "#3d7a8f" };
  }
  return { glyph: speaker.slice(0, 1) || "人", tone: "#8f4b32" };
}

/**
 * Parses multi-voice Keeper narration (`【守秘人】`, `【NPC】`, `【直觉·技能】`, `【抉择】`, `【建议行动】`)
 * into typed theater beats plus extracted 1-click action choices.
 * Plain legacy prose without tags gracefully stays as a single `prose` beat.
 */
export function parseTheaterNarration(raw: string): ParsedTheaterNarration {
  const trimmed = raw.trim();
  if (!trimmed) return { beats: [], choices: [] };

  // Split out trailing `【建议行动】` section first
  let narrativePart = trimmed;
  let choicesPart = "";
  const choiceMarkerIdx = trimmed.indexOf("【建议行动】");
  if (choiceMarkerIdx !== -1) {
    narrativePart = trimmed.slice(0, choiceMarkerIdx).trim();
    choicesPart = trimmed.slice(choiceMarkerIdx + "【建议行动】".length).trim();
  }

  const choices: string[] = [];
  if (choicesPart) {
    for (const rawLine of choicesPart.split(/\r?\n/)) {
      const cleaned = cleanPlainText(rawLine)
        .replace(/^\s*(?:\d+[.、)）]\s*|[-•·]\s*)/, "")
        .trim();
      if (cleaned.length >= 2) {
        choices.push(cleaned);
      }
    }
  }

  // Match all `【...】` speaker tags inside the narrative body
  const tagRegex = /【([^】]{1,24})】/g;
  const matches = [...narrativePart.matchAll(tagRegex)];

  if (matches.length === 0) {
    const plain = cleanPlainText(narrativePart);
    return {
      beats: plain ? [{ kind: "prose", speaker: "", body: plain }] : [],
      choices,
    };
  }

  const beats: TheaterBeat[] = [];
  const prefix = cleanPlainText(narrativePart.slice(0, matches[0].index ?? 0));
  if (prefix) {
    beats.push({ kind: "prose", speaker: "", body: prefix });
  }

  for (let i = 0; i < matches.length; i += 1) {
    const match = matches[i];
    const tag = (match[1] ?? "").trim();
    const startIdx = (match.index ?? 0) + match[0].length;
    const endIdx = i + 1 < matches.length ? (matches[i + 1].index ?? narrativePart.length) : narrativePart.length;
    const body = cleanPlainText(narrativePart.slice(startIdx, endIdx).replace(/^[：:]\s*/, ""));
    if (!body) continue;

    if (tag === "守秘人" || tag === "KP" || tag === "主持人") {
      beats.push({ kind: "kp", speaker: "守秘人 · KP", body });
    } else if (tag.startsWith("直觉") || tag.startsWith("灵感") || tag.startsWith("技能")) {
      const skillName = tag.replace(/^直觉[·•:：\-\s]*/, "").trim() || "直觉";
      beats.push({ kind: "skill", speaker: `直觉 · ${skillName}`, body });
    } else if (tag === "抉择" || tag === "焦点" || tag === "局势") {
      beats.push({ kind: "hook", speaker: "抉择时刻", body });
    } else {
      beats.push({ kind: "npc", speaker: tag, body });
    }
  }

  return { beats, choices };
}

interface TheaterTranscriptBlockProps {
  text: string;
  isStreaming?: boolean;
}

export function TheaterTranscriptBlock({ text, isStreaming = false }: TheaterTranscriptBlockProps) {
  const { beats } = parseTheaterNarration(text);

  if (beats.length === 0) {
    return null;
  }

  // Plain prose fallback for legacy/mock transcripts without speaker tags
  if (beats.length === 1 && beats[0].kind === "prose") {
    return (
      <div className="vellum-prose whitespace-pre-line text-[var(--vellum-ink)]">
        {beats[0].body}
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {beats.map((beat, idx) => {
        if (beat.kind === "kp") {
          return (
            <div
              key={idx}
              className="rounded-[var(--vellum-radius-md)] border-l-[3px] border-[var(--vellum-spk-kp)] bg-[var(--vellum-spk-kp-bg)] px-4 py-3"
            >
              <div className="mb-1 inline-flex items-center gap-1.5 text-[11px] font-bold tracking-[0.12em] text-[var(--vellum-spk-kp)]">
                <Flame className="h-3.5 w-3.5" />
                <span>{beat.speaker}</span>
              </div>
              <p className="vellum-prose whitespace-pre-line text-[var(--vellum-ink)]">{beat.body}</p>
            </div>
          );
        }

        if (beat.kind === "npc") {
          const { glyph, tone } = npcMonogram(beat.speaker);
          return (
            <div
              key={idx}
              className="flex items-start gap-3 rounded-[var(--vellum-radius-md)] border border-[var(--vellum-spk-npc)]/25 border-l-[3px] border-l-[var(--vellum-spk-npc)] bg-[var(--vellum-spk-npc-bg)] px-4 py-3"
            >
              <div
                className="vellum-prose mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-full border-2 border-[var(--vellum-brass)]/75 text-sm font-bold text-[#fff8ec] shadow-xs"
                style={{ backgroundColor: tone }}
                aria-hidden="true"
              >
                {glyph}
              </div>
              <div className="min-w-0 flex-1">
                <div className="text-xs font-bold tracking-wide text-[var(--vellum-spk-npc)]">
                  {beat.speaker}
                </div>
                <p className="vellum-prose mt-1 whitespace-pre-line text-[var(--vellum-ink)]">
                  {beat.body}
                </p>
              </div>
            </div>
          );
        }

        if (beat.kind === "skill") {
          return (
            <div
              key={idx}
              className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-spk-skill)]/30 border-l-[3px] border-l-[var(--vellum-spk-skill)] bg-[var(--vellum-spk-skill-bg)] px-4 py-2.5"
            >
              <div className="mb-1 inline-flex items-center gap-1.5 text-[11px] font-bold tracking-[0.08em] text-[var(--vellum-spk-skill)]">
                <Brain className="h-3.5 w-3.5" />
                <span>{beat.speaker}</span>
              </div>
              <p className="text-sm italic leading-7 text-[var(--vellum-ink)]">{beat.body}</p>
            </div>
          );
        }

        if (beat.kind === "hook") {
          return (
            <div
              key={idx}
              className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-spk-hook)]/45 bg-[var(--vellum-spk-hook-bg)] px-4 py-2.5"
            >
              <div className="mb-1 inline-flex items-center gap-1.5 text-[11px] font-bold tracking-[0.1em] text-[var(--vellum-spk-hook)]">
                <Compass className="h-3.5 w-3.5" />
                <span>{beat.speaker}</span>
              </div>
              <p className="text-sm font-medium leading-7 text-[var(--vellum-ink)]">{beat.body}</p>
            </div>
          );
        }

        return (
          <div key={idx} className="vellum-prose whitespace-pre-line text-[var(--vellum-ink)]">
            {beat.body}
          </div>
        );
      })}

      {isStreaming ? (
        <span
          aria-hidden="true"
          className="inline-flex items-center gap-1 text-[11px] text-[var(--vellum-brass)]"
        >
          <Sparkles className="h-3 w-3 animate-pulse" />
          <span>守秘人叙述中……</span>
        </span>
      ) : null}
    </div>
  );
}
