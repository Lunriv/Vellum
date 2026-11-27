import { BookOpen, Compass, HelpCircle, Moon, ShieldAlert, Sun } from "lucide-react";
import type { ExperienceMode } from "../../api/types";
import { useGameStore } from "../../stores/gameStore";

interface TopBarProps {
  sceneTitle?: string;
  cluesCount?: number;
  hasPendingInterrupt?: boolean;
  sessionEnded?: boolean;
  onExperienceModeChange?: (mode: ExperienceMode) => void;
  onOpenDebrief?: () => void;
  onNewAdventure?: () => void;
}

const MODE_OPTIONS: Array<{ value: ExperienceMode; label: string }> = [
  { value: "guided", label: "引导" },
  { value: "classic", label: "经典" },
  { value: "immersive", label: "沉浸" },
];

export function TopBar({
  sceneTitle,
  cluesCount = 0,
  hasPendingInterrupt = false,
  sessionEnded = false,
  onExperienceModeChange,
  onOpenDebrief,
  onNewAdventure,
}: TopBarProps) {
  const theme = useGameStore((s) => s.theme);
  const toggleTheme = useGameStore((s) => s.toggleTheme);
  const experienceMode = useGameStore((s) => s.experienceMode);
  const lowSanEffectsEnabled = useGameStore((s) => s.lowSanEffectsEnabled);
  const setLowSanEffectsEnabled = useGameStore((s) => s.setLowSanEffectsEnabled);
  const activeDrawer = useGameStore((s) => s.activeDrawer);
  const setActiveDrawer = useGameStore((s) => s.setActiveDrawer);
  const connectionStatus = useGameStore((s) => s.connectionStatus);

  const toggleDrawer = (drawer: "dossier" | "evidence" | "ooc") => {
    setActiveDrawer(activeDrawer === drawer ? null : drawer);
  };

  return (
    <header className="sticky top-0 z-20 border-b border-[var(--vellum-wood-border)]/80 bg-[var(--vellum-wood)]/95 shadow-[var(--vellum-shadow-sm)] backdrop-blur-sm">
      <div className="mx-auto flex max-w-[1440px] flex-wrap items-center justify-between gap-3 px-4 py-2.5 sm:px-6">
        <div className="flex min-w-0 items-center gap-3">
          <div className="inline-flex items-center gap-2">
            <span
              className="flex h-6 w-6 items-center justify-center rounded-full border border-[var(--vellum-brass)] bg-[#8f2424] text-[10px] font-bold text-[#fdf7ea]"
              aria-hidden="true"
            >
              V
            </span>
            <span className="vellum-prose text-sm font-bold tracking-[0.16em] text-[var(--vellum-brass)]">
              VELLUM
            </span>
          </div>
          {sceneTitle ? (
            <>
              <span className="text-[var(--vellum-wood-border)]" aria-hidden="true">
                ·
              </span>
              <span className="truncate text-xs font-semibold text-[var(--vellum-ink)] sm:text-sm">
                {sceneTitle}
              </span>
            </>
          ) : null}
          {connectionStatus === "reconnecting" ? (
            <span className="rounded-full border border-[var(--vellum-warning)]/40 bg-[var(--vellum-warning-soft)] px-2 py-0.5 text-[11px] font-medium text-[var(--vellum-warning)]">
              正在重连…
            </span>
          ) : null}
        </div>

        <div className="flex flex-wrap items-center gap-1.5 sm:gap-2">
          {onExperienceModeChange ? (
            <div
              className="inline-flex rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-wood-border)] bg-[var(--vellum-surface)] p-0.5"
              role="group"
              aria-label="体验模式"
            >
              {MODE_OPTIONS.map((opt) => {
                const active = experienceMode === opt.value;
                return (
                  <button
                    key={opt.value}
                    type="button"
                    onClick={() => onExperienceModeChange(opt.value)}
                    aria-pressed={active}
                    className={`rounded-[3px] px-2.5 py-1 text-xs font-medium transition-colors ${
                      active
                        ? "bg-[var(--vellum-accent)] text-[var(--vellum-accent-ink)]"
                        : "text-[var(--vellum-muted)] hover:text-[var(--vellum-ink)]"
                    }`}
                  >
                    {opt.label}
                  </button>
                );
              })}
            </div>
          ) : null}

          {sceneTitle ? (
            <>
              <button
                type="button"
                onClick={() => toggleDrawer("dossier")}
                aria-pressed={activeDrawer === "dossier"}
                className={`inline-flex items-center gap-1.5 rounded-[var(--vellum-radius-sm)] border px-2.5 py-1.5 text-xs font-medium transition-colors lg:hidden ${
                  activeDrawer === "dossier"
                    ? "border-[var(--vellum-accent)] bg-[var(--vellum-accent-soft)] text-[var(--vellum-accent)]"
                    : "border-[var(--vellum-wood-border)] bg-[var(--vellum-surface)] text-[var(--vellum-ink)] hover:bg-[var(--vellum-elevated)]"
                }`}
              >
                <Compass className="h-3.5 w-3.5 text-[var(--vellum-brass)]" />
                <span>沙盘</span>
              </button>

              <button
                type="button"
                onClick={() => toggleDrawer("evidence")}
                aria-pressed={activeDrawer === "evidence"}
                className={`inline-flex items-center gap-1.5 rounded-[var(--vellum-radius-sm)] border px-2.5 py-1.5 text-xs font-medium transition-colors ${
                  activeDrawer === "evidence"
                    ? "border-[var(--vellum-accent)] bg-[var(--vellum-accent-soft)] text-[var(--vellum-accent)]"
                    : "border-[var(--vellum-wood-border)] bg-[var(--vellum-surface)] text-[var(--vellum-ink)] hover:bg-[var(--vellum-elevated)]"
                }`}
              >
                <BookOpen className="h-3.5 w-3.5 text-[var(--vellum-brass)]" />
                <span>线索板</span>
                {cluesCount > 0 ? (
                  <span className="vellum-mono rounded bg-[var(--vellum-accent)] px-1.5 py-0.2 text-[10px] font-bold text-[var(--vellum-accent-ink)]">
                    {cluesCount}
                  </span>
                ) : null}
              </button>

              <button
                type="button"
                onClick={() => toggleDrawer("ooc")}
                aria-pressed={activeDrawer === "ooc"}
                className={`inline-flex items-center gap-1.5 rounded-[var(--vellum-radius-sm)] border px-2.5 py-1.5 text-xs font-medium transition-colors ${
                  activeDrawer === "ooc"
                    ? "border-[var(--vellum-accent)] bg-[var(--vellum-accent-soft)] text-[var(--vellum-accent)]"
                    : "border-[var(--vellum-wood-border)] bg-[var(--vellum-surface)] text-[var(--vellum-ink)] hover:bg-[var(--vellum-elevated)]"
                }`}
                title={hasPendingInterrupt ? "检定等待期间也可随时询问规则" : "场外规则与状态查询"}
              >
                <HelpCircle className="h-3.5 w-3.5" />
                <span>OOC</span>
              </button>
            </>
          ) : null}

          {sessionEnded && onOpenDebrief ? (
            <button
              type="button"
              onClick={onOpenDebrief}
              className="rounded-[var(--vellum-radius-sm)] bg-[var(--vellum-accent)] px-3 py-1.5 text-xs font-semibold text-[var(--vellum-accent-ink)] hover:bg-[var(--vellum-accent-hover)]"
            >
              查看结案复盘
            </button>
          ) : null}

          <button
            type="button"
            onClick={() => setLowSanEffectsEnabled(!lowSanEffectsEnabled)}
            aria-pressed={lowSanEffectsEnabled}
            title={lowSanEffectsEnabled ? "点击关闭低理智视觉效果" : "点击开启低理智视觉效果"}
            className="inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-wood-border)] bg-[var(--vellum-surface)] px-2 py-1.5 text-xs text-[var(--vellum-muted)] hover:bg-[var(--vellum-elevated)] hover:text-[var(--vellum-ink)]"
          >
            <ShieldAlert className="h-3.5 w-3.5" />
            <span className="hidden sm:inline">
              {lowSanEffectsEnabled ? "理智特效开" : "理智特效关"}
            </span>
          </button>

          <button
            type="button"
            onClick={toggleTheme}
            aria-label={theme === "light" ? "切换到雨夜夜间模式" : "切换到纸墨白天模式"}
            className="inline-flex items-center gap-1.5 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-wood-border)] bg-[var(--vellum-surface)] px-2.5 py-1.5 text-xs font-medium text-[var(--vellum-ink)] hover:border-[var(--vellum-brass)]"
          >
            {theme === "light" ? <Moon className="h-3.5 w-3.5" /> : <Sun className="h-3.5 w-3.5" />}
            <span>{theme === "light" ? "暗桌" : "亮桌"}</span>
          </button>

          {onNewAdventure ? (
            <button
              type="button"
              onClick={onNewAdventure}
              className="rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-wood-border)] bg-[var(--vellum-surface)] px-2.5 py-1.5 text-xs text-[var(--vellum-muted)] hover:bg-[var(--vellum-elevated)] hover:text-[var(--vellum-ink)]"
            >
              重新选角
            </button>
          ) : null}
        </div>
      </div>
    </header>
  );
}
