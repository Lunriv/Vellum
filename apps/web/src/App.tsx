import { useEffect, useMemo, useState } from "react";
import { AlertCircle, RefreshCw } from "lucide-react";
import type { RollAttemptDTO, VisibleSceneDTO } from "./api/types";
import { AtmosphereLayer } from "./components/atmosphere/AtmosphereLayer";
import { CaseDebriefView } from "./components/debrief/CaseDebriefView";
import { EvidenceBoardDrawer } from "./components/evidence/EvidenceBoardDrawer";
import { TopBar } from "./components/layout/TopBar";
import { ActionComposer } from "./components/narrative/ActionComposer";
import { NarrativeStage } from "./components/narrative/NarrativeStage";
import { parseTheaterNarration } from "./components/narrative/TheaterTranscriptBlock";
import { OocDrawer } from "./components/ooc/OocDrawer";
import { TabletopBattlefieldPane } from "./components/tabletop/TabletopBattlefieldPane";
import { TurnZeroSetup } from "./components/turn0/TurnZeroSetup";
import { useSessionSync } from "./hooks/useSessionSync";
import { useGameStore } from "./stores/gameStore";

const VALID_ATMOSPHERES = new Set<VisibleSceneDTO["atmosphere"]>([
  "rain_manor",
  "candlelight",
  "fog",
  "san_strain",
]);

function extractRollFromPayload(payload: Record<string, unknown>): RollAttemptDTO | null {
  if (typeof payload.rollValue === "number" && typeof payload.skill === "string") {
    return payload as unknown as RollAttemptDTO;
  }
  return null;
}

export function App() {
  const theme = useGameStore((s) => s.theme);
  const experienceMode = useGameStore((s) => s.experienceMode);
  const activeDrawer = useGameStore((s) => s.activeDrawer);
  const setActiveDrawer = useGameStore((s) => s.setActiveDrawer);
  const clearActiveTurn = useGameStore((s) => s.clearActiveTurn);

  const {
    sessionId,
    sessionState,
    isLoadingSession,
    isSessionError,
    sessionError,
    refetchSessionState,
    activeTurn,
    pendingInterrupt,
    isResolvingInterrupt,
    interruptError,
    startSessionFromTurn0,
    submitTurn,
    resolveInterrupt,
    changeExperienceMode,
    resetToTurnZero,
  } = useSessionSync();

  const [composerDraft, setComposerDraft] = useState("");
  const [debriefOpen, setDebriefOpen] = useState(false);
  const [retryingTurn, setRetryingTurn] = useState(false);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  const effectiveAtmosphere = useMemo<VisibleSceneDTO["atmosphere"]>(() => {
    if (activeTurn) {
      for (let i = activeTurn.immediateDirectives.length - 1; i >= 0; i -= 1) {
        const d = activeTurn.immediateDirectives[i];
        if (d.directiveType === "atmosphere_shift" && typeof d.payload.theme === "string") {
          const candidate = d.payload.theme as VisibleSceneDTO["atmosphere"];
          if (VALID_ATMOSPHERES.has(candidate)) {
            return candidate;
          }
        }
      }
    }
    return sessionState?.currentScene.atmosphere ?? "rain_manor";
  }, [activeTurn, sessionState?.currentScene.atmosphere]);

  const sessionEnded = sessionState?.session.status === "ended";

  const isTurnBusy = Boolean(
    activeTurn &&
      !["completed", "cancelled", "failed", "interrupted_before_commit"].includes(
        activeTurn.status,
      ) &&
      !pendingInterrupt,
  );

  // Extract latest Keeper contextual 1-click choices from the most recent completed narration
  const latestDynamicChoices = useMemo<string[]>(() => {
    if (activeTurn?.narrationComplete && activeTurn.narrationText) {
      const parsed = parseTheaterNarration(activeTurn.narrationText);
      if (parsed.choices.length > 0) return parsed.choices;
    }
    const list = sessionState?.transcript ?? [];
    for (let i = list.length - 1; i >= 0; i -= 1) {
      const parsed = parseTheaterNarration(list[i].narration);
      if (parsed.choices.length > 0) return parsed.choices;
    }
    return [];
  }, [activeTurn?.narrationComplete, activeTurn?.narrationText, sessionState?.transcript]);

  // Keep the physical felt Dice Tray synced with the latest authoritative dice roll
  const lastRoll = useMemo<RollAttemptDTO | null>(() => {
    if (activeTurn) {
      const allActiveDirs = [
        ...activeTurn.immediateDirectives,
        ...activeTurn.releasedAfterNarration,
      ];
      for (let i = allActiveDirs.length - 1; i >= 0; i -= 1) {
        if (allActiveDirs[i].directiveType === "inline_dice") {
          const found = extractRollFromPayload(allActiveDirs[i].payload);
          if (found) return found;
        }
      }
    }
    const list = sessionState?.transcript ?? [];
    for (let i = list.length - 1; i >= 0; i -= 1) {
      const dirs = list[i].directives ?? [];
      for (let j = dirs.length - 1; j >= 0; j -= 1) {
        if (dirs[j].directiveType === "inline_dice") {
          const found = extractRollFromPayload(dirs[j].payload);
          if (found) return found;
        }
      }
    }
    return null;
  }, [activeTurn, sessionState?.transcript]);

  const handleRetryInterruptedAction = async (actionInput: string) => {
    const trimmed = actionInput.trim();
    if (!trimmed || retryingTurn) return;
    setRetryingTurn(true);
    try {
      await submitTurn(trimmed);
    } finally {
      setRetryingTurn(false);
    }
  };

  const handleDirectTabletopAction = (actionText: string) => {
    if (Boolean(pendingInterrupt) || Boolean(sessionEnded) || isTurnBusy) return;
    void submitTurn(actionText);
  };

  const handleNewAdventure = () => {
    setDebriefOpen(false);
    setComposerDraft("");
    resetToTurnZero();
  };

  return (
    <AtmosphereLayer
      atmosphere={effectiveAtmosphere}
      san={sessionState?.investigator.san}
      sanMax={sessionState?.investigator.sanMax}
    >
      <TopBar
        sceneTitle={sessionId ? (sessionState?.currentScene.title ?? "绯雨庄园") : undefined}
        cluesCount={sessionState?.clues.length ?? 0}
        hasPendingInterrupt={Boolean(pendingInterrupt)}
        sessionEnded={sessionEnded}
        onExperienceModeChange={sessionId ? (mode) => void changeExperienceMode(mode) : undefined}
        onOpenDebrief={sessionEnded ? () => setDebriefOpen(true) : undefined}
        onNewAdventure={sessionId ? handleNewAdventure : undefined}
      />

      {!sessionId ? (
        <TurnZeroSetup
          onSessionCreated={(res) => {
            setDebriefOpen(false);
            setComposerDraft("");
            startSessionFromTurn0(res);
          }}
        />
      ) : debriefOpen && sessionEnded ? (
        <CaseDebriefView
          sessionId={sessionId}
          onBackToTranscript={() => setDebriefOpen(false)}
          onStartNewAdventure={handleNewAdventure}
        />
      ) : isSessionError && !sessionState ? (
        <main className="mx-auto max-w-xl px-4 py-12 sm:px-6">
          <section
            role="alert"
            className="rounded-[var(--vellum-radius-md)] border border-[var(--vellum-danger)]/40 bg-[var(--vellum-danger-soft)] p-6 text-[var(--vellum-ink)]"
          >
            <div className="flex items-start gap-3">
              <AlertCircle className="mt-0.5 h-5 w-5 shrink-0 text-[var(--vellum-danger)]" />
              <div className="flex-1">
                <h1 className="text-sm font-semibold">无法恢复当前调查进度</h1>
                <p className="mt-1 text-xs leading-6 text-[var(--vellum-muted)]">
                  {sessionError instanceof Error && sessionError.message
                    ? `读取会话状态未成功（${sessionError.message}）。你可以尝试重新连接，或重新选角开启新的调查。`
                    : "当前无法连接守秘人服务，或本地旧存档已不存在。"}
                </p>
                <div className="mt-4 flex flex-wrap items-center gap-2">
                  <button
                    type="button"
                    onClick={() => void refetchSessionState()}
                    className="inline-flex items-center gap-1.5 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border-strong)] bg-[var(--vellum-surface)] px-3 py-1.5 text-xs font-medium text-[var(--vellum-ink)] hover:bg-[var(--vellum-elevated)]"
                  >
                    <RefreshCw className="h-3.5 w-3.5" />
                    <span>重新连接</span>
                  </button>
                  <button
                    type="button"
                    onClick={handleNewAdventure}
                    className="rounded-[var(--vellum-radius-sm)] bg-[var(--vellum-accent)] px-3 py-1.5 text-xs font-semibold text-[var(--vellum-accent-ink)] hover:bg-[var(--vellum-accent-hover)]"
                  >
                    重新选角
                  </button>
                </div>
              </div>
            </div>
          </section>
        </main>
      ) : isLoadingSession && !sessionState && !activeTurn ? (
        <main className="mx-auto max-w-2xl px-4 py-12 sm:px-6">
          <p className="vellum-prose text-sm text-[var(--vellum-muted)]">
            正在铺开庄园沙盘与过往调查记录……
          </p>
        </main>
      ) : (
        <div className="mx-auto flex w-full max-w-[1440px] flex-1">
          {sessionState ? (
            <TabletopBattlefieldPane
              investigator={sessionState.investigator}
              currentScene={sessionState.currentScene}
              mapRooms={sessionState.mapRooms}
              visibleClocks={sessionState.visibleClocks}
              pendingInterrupt={pendingInterrupt}
              lastRoll={lastRoll}
              isResolvingInterrupt={isResolvingInterrupt}
              mobileDrawerOpen={activeDrawer === "dossier"}
              actionLocked={Boolean(pendingInterrupt) || Boolean(sessionEnded) || isTurnBusy}
              onCloseMobileDrawer={() => setActiveDrawer(null)}
              onExecuteAction={handleDirectTabletopAction}
              onQuickFillAction={(actionText) => setComposerDraft(actionText)}
              onResolveInterrupt={(payload) => void resolveInterrupt(payload)}
            />
          ) : null}

          <div className="flex min-w-0 flex-1 flex-col justify-between">
            <main className="flex-1 px-4 sm:px-6">
              <NarrativeStage
                transcript={sessionState?.transcript ?? []}
                activeTurn={activeTurn}
                pendingInterrupt={pendingInterrupt}
                experienceMode={experienceMode}
                isResolvingInterrupt={isResolvingInterrupt}
                interruptError={interruptError}
                isRetryingTurn={retryingTurn}
                onResolveInterrupt={(payload) => void resolveInterrupt(payload)}
                onRetryInterruptedAction={(input) => void handleRetryInterruptedAction(input)}
                onDiscardInterruptedTurn={clearActiveTurn}
              />
            </main>

            <ActionComposer
              draft={composerDraft}
              onDraftChange={setComposerDraft}
              onSubmitAction={submitTurn}
              experienceMode={experienceMode}
              dynamicChoices={latestDynamicChoices}
              guidedPrompts={sessionState?.guidedPrompts}
              pendingInterrupt={pendingInterrupt}
              isTurnBusy={isTurnBusy}
              sessionEnded={Boolean(sessionEnded)}
              onOpenOoc={() => setActiveDrawer("ooc")}
              onOpenDebrief={() => setDebriefOpen(true)}
            />
          </div>
        </div>
      )}

      {sessionId ? (
        <>
          <EvidenceBoardDrawer
            open={activeDrawer === "evidence"}
            sessionId={sessionId}
            fallbackClues={sessionState?.clues}
            fallbackHandouts={sessionState?.handouts}
            onClose={() => setActiveDrawer(null)}
          />
          <OocDrawer
            open={activeDrawer === "ooc"}
            sessionId={sessionId}
            hasPendingInterrupt={Boolean(pendingInterrupt)}
            onClose={() => setActiveDrawer(null)}
          />
        </>
      ) : null}
    </AtmosphereLayer>
  );
}
