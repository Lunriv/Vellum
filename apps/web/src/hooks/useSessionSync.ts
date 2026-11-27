import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ApiError, vellumApi } from "../api/client";
import { subscribeTurnEvents } from "../api/sse";
import type {
  ClientVisibleStateDTO,
  ClientVisibleStatePatch,
  CreateSessionResponse,
  ExperienceMode,
  ResolveInterruptRequest,
  TurnStatus,
} from "../api/types";
import { useGameStore } from "../stores/gameStore";

const HALTED_OR_TERMINAL_STATUSES = new Set<TurnStatus>([
  "completed",
  "cancelled",
  "failed",
  "interrupted_before_commit",
]);

function applyPatchToSessionState(
  prev: ClientVisibleStateDTO | undefined,
  patch: ClientVisibleStatePatch,
): ClientVisibleStateDTO | undefined {
  if (!prev) return prev;
  const mergedClues = patch.addedClues
    ? [
        ...prev.clues,
        ...patch.addedClues.filter((c) => !prev.clues.some((existing) => existing.clueId === c.clueId)),
      ]
    : prev.clues;

  const mergedHandouts = patch.addedHandouts
    ? [
        ...prev.handouts,
        ...patch.addedHandouts.filter(
          (h) => !prev.handouts.some((existing) => existing.handoutId === h.handoutId),
        ),
      ]
    : prev.handouts;

  return {
    ...prev,
    session: {
      ...prev.session,
      sessionVersion: patch.sessionVersion ?? prev.session.sessionVersion,
      status: patch.sessionStatus ?? prev.session.status,
    },
    moduleTitle: patch.moduleTitle ?? prev.moduleTitle,
    investigator: patch.investigator ? { ...prev.investigator, ...patch.investigator } : prev.investigator,
    currentScene: patch.currentScene ?? prev.currentScene,
    mapRooms: patch.mapRooms ?? prev.mapRooms,
    mapEdges: patch.mapEdges ?? prev.mapEdges,
    worldTags: patch.worldTags ?? prev.worldTags,
    visibleClocks: patch.visibleClocks ?? prev.visibleClocks,
    clues: mergedClues,
    handouts: mergedHandouts,
    pendingInterrupt:
      patch.pendingInterrupt !== undefined ? patch.pendingInterrupt : prev.pendingInterrupt,
    guidedPrompts: patch.guidedPrompts ?? prev.guidedPrompts,
  };
}

export interface SubmitTurnResult {
  ok: boolean;
  conflict?: boolean;
  errorMessage?: string;
}

export function useSessionSync() {
  const queryClient = useQueryClient();

  const sessionId = useGameStore((s) => s.sessionId);
  const activeTurn = useGameStore((s) => s.activeTurn);
  const pendingInterrupt = useGameStore((s) => s.pendingInterrupt);
  const newlyReleasedDirectiveIds = useGameStore((s) => s.newlyReleasedDirectiveIds);

  const setSessionId = useGameStore((s) => s.setSessionId);
  const setExperienceMode = useGameStore((s) => s.setExperienceMode);
  const setActiveDrawer = useGameStore((s) => s.setActiveDrawer);
  const setConnectionStatus = useGameStore((s) => s.setConnectionStatus);
  const setPendingInterrupt = useGameStore((s) => s.setPendingInterrupt);
  const beginTurn = useGameStore((s) => s.beginTurn);
  const hydrateTurnFromSnapshot = useGameStore((s) => s.hydrateTurnFromSnapshot);
  const consumeNewlyReleasedDirectives = useGameStore((s) => s.consumeNewlyReleasedDirectives);
  const clearActiveTurn = useGameStore((s) => s.clearActiveTurn);

  const [isResolvingInterrupt, setIsResolvingInterrupt] = useState(false);
  const [interruptError, setInterruptError] = useState<string | null>(null);

  const syncedModeSessionRef = useRef<string | null>(null);
  const hydratedTurnKeyRef = useRef<string | null>(null);

  const sessionQuery = useQuery({
    queryKey: ["sessionState", sessionId],
    queryFn: () => vellumApi.getSessionState(sessionId!),
    enabled: Boolean(sessionId),
    retry: (failureCount, err) => {
      if (err instanceof ApiError && err.status === 404) return false;
      return failureCount < 2;
    },
  });

  const sessionState = sessionQuery.data;

  // Automatically clear stale localStorage sessionId when the backend reports 404 (e.g. fresh local DB)
  useEffect(() => {
    if (
      sessionId &&
      sessionQuery.isError &&
      sessionQuery.error instanceof ApiError &&
      sessionQuery.error.status === 404
    ) {
      syncedModeSessionRef.current = null;
      hydratedTurnKeyRef.current = null;
      setActiveDrawer(null);
      setConnectionStatus("idle");
      setSessionId(null);
    }
  }, [
    sessionId,
    sessionQuery.isError,
    sessionQuery.error,
    setActiveDrawer,
    setConnectionStatus,
    setSessionId,
  ]);

  // Sync experienceMode and pendingInterrupt when sessionState arrives/updates
  useEffect(() => {
    if (!sessionState || !sessionId) return;

    if (syncedModeSessionRef.current !== sessionId) {
      syncedModeSessionRef.current = sessionId;
      setExperienceMode(sessionState.session.experienceMode);
    }

    setPendingInterrupt(sessionState.pendingInterrupt);

    // Clear activeTurn once its committed entry is present in transcript
    const currentTurn = useGameStore.getState().activeTurn;
    if (
      currentTurn &&
      currentTurn.status === "completed" &&
      sessionState.transcript.some((entry) => entry.turnId === currentTurn.turnId) &&
      sessionState.session.activeTurnId !== currentTurn.turnId
    ) {
      clearActiveTurn();
    }
  }, [sessionId, sessionState, setExperienceMode, setPendingInterrupt, clearActiveTurn]);

  // Recover activeTurn snapshot (including persistent after_narration directives) on page load / refresh
  useEffect(() => {
    const activeTurnId = sessionState?.session.activeTurnId;
    if (!sessionId || !activeTurnId) return;

    const currentTurn = useGameStore.getState().activeTurn;
    if (currentTurn && currentTurn.turnId === activeTurnId) return;

    const hydrationKey = `${sessionId}:${activeTurnId}`;
    if (hydratedTurnKeyRef.current === hydrationKey) return;
    hydratedTurnKeyRef.current = hydrationKey;

    let cancelled = false;
    void vellumApi
      .getTurn(sessionId, activeTurnId)
      .then((snap) => {
        if (cancelled) return;
        hydrateTurnFromSnapshot(
          snap.turnId,
          snap.playerInput ?? null,
          snap.status,
          snap.narrationGeneration,
          snap.narrationText,
          snap.directives,
          snap.pendingInterrupt,
        );
      })
      .catch(() => {
        if (!cancelled) {
          hydratedTurnKeyRef.current = null;
        }
      });

    return () => {
      cancelled = true;
    };
  }, [sessionId, sessionState?.session.activeTurnId, hydrateTurnFromSnapshot]);

  // Subscribe to SSE stream for the currently active non-terminal turn
  const streamTurnId =
    sessionId && activeTurn && !HALTED_OR_TERMINAL_STATUSES.has(activeTurn.status)
      ? activeTurn.turnId
      : null;

  useEffect(() => {
    if (!sessionId || !streamTurnId) {
      setConnectionStatus("idle");
      return;
    }

    const handle = subscribeTurnEvents(sessionId, streamTurnId, {
      onStatusChange: (status) => {
        setConnectionStatus(status);
      },
      onEvent: (event) => {
        useGameStore.getState().applyStreamEvent(event);

        if (event.type === "state_delta") {
          queryClient.setQueryData<ClientVisibleStateDTO>(["sessionState", sessionId], (prev) =>
            applyPatchToSessionState(prev, event.patch),
          );
          if (event.patch.pendingInterrupt !== undefined) {
            setPendingInterrupt(event.patch.pendingInterrupt);
          }
          void queryClient.invalidateQueries({ queryKey: ["sessionState", sessionId] });
          if (event.patch.addedClues?.length || event.patch.addedHandouts?.length) {
            void queryClient.invalidateQueries({ queryKey: ["evidence", sessionId] });
          }
        } else if (event.type === "turn_complete") {
          void queryClient.invalidateQueries({ queryKey: ["sessionState", sessionId] });
          void queryClient.invalidateQueries({ queryKey: ["evidence", sessionId] });
        } else if (event.type === "stream_reset") {
          void queryClient.invalidateQueries({ queryKey: ["sessionState", sessionId] });
        }
      },
    });

    return () => {
      handle.close();
    };
  }, [sessionId, streamTurnId, queryClient, setConnectionStatus, setPendingInterrupt]);

  // Acknowledge newly released after_narration directives so refreshes won't replay first-reveal animation
  useEffect(() => {
    if (!sessionId || newlyReleasedDirectiveIds.length === 0) return;
    const ids = consumeNewlyReleasedDirectives();
    for (const directiveId of ids) {
      void vellumApi.ackDirective(sessionId, directiveId).catch(() => {
        // Best-effort presentation ack; authoritative state remains intact
      });
    }
  }, [sessionId, newlyReleasedDirectiveIds, consumeNewlyReleasedDirectives]);

  const startSessionFromTurn0 = useCallback(
    (res: CreateSessionResponse) => {
      syncedModeSessionRef.current = res.session.sessionId;
      hydratedTurnKeyRef.current = `${res.session.sessionId}:${res.turn0Id}`;
      setSessionId(res.session.sessionId);
      setExperienceMode(res.session.experienceMode);
      beginTurn(res.turn0Id, null, "narrating");
      void queryClient.invalidateQueries({ queryKey: ["sessionState", res.session.sessionId] });
    },
    [setSessionId, setExperienceMode, beginTurn, queryClient],
  );

  const resyncSession = useCallback(async () => {
    if (!sessionId) return;
    const refreshed = await queryClient.fetchQuery({
      queryKey: ["sessionState", sessionId],
      queryFn: () => vellumApi.getSessionState(sessionId),
    });
    if (refreshed.session.activeTurnId) {
      try {
        const snap = await vellumApi.getTurn(sessionId, refreshed.session.activeTurnId);
        hydrateTurnFromSnapshot(
          snap.turnId,
          snap.playerInput ?? null,
          snap.status,
          snap.narrationGeneration,
          snap.narrationText,
          snap.directives,
          snap.pendingInterrupt,
        );
      } catch {
        // Ignore transient turn fetch errors during conflict resync
      }
    }
  }, [sessionId, queryClient, hydrateTurnFromSnapshot]);

  const submitTurn = useCallback(
    async (input: string): Promise<SubmitTurnResult> => {
      if (!sessionId) {
        return { ok: false, errorMessage: "当前尚未建立有效调查会话。" };
      }
      try {
        const res = await vellumApi.createTurn(sessionId, { input });
        hydratedTurnKeyRef.current = `${sessionId}:${res.turnId}`;
        beginTurn(res.turnId, input, res.status);
        return { ok: true };
      } catch (err) {
        if (err instanceof ApiError && err.status === 409) {
          void resyncSession();
          return {
            ok: false,
            conflict: true,
            errorMessage: "当前已有正在结算的行动或待决检定，已为你同步最新现场状态。",
          };
        }
        return {
          ok: false,
          errorMessage:
            err instanceof Error && err.message ? err.message : "行动提交未成功，请稍后重试。",
        };
      }
    },
    [sessionId, beginTurn, resyncSession],
  );

  const resolveInterrupt = useCallback(
    async (payload: ResolveInterruptRequest) => {
      if (!sessionId || !pendingInterrupt) return;
      setIsResolvingInterrupt(true);
      setInterruptError(null);

      try {
        const res = await vellumApi.resolveInterrupt(
          sessionId,
          pendingInterrupt.interruptId,
          payload,
        );

        setPendingInterrupt(res.pendingInterrupt);

        if (payload.action === "cancel" || res.status === "cancelled") {
          clearActiveTurn();
        } else if (payload.action === "revise_action" && res.replacementTurnId) {
          hydratedTurnKeyRef.current = `${sessionId}:${res.replacementTurnId}`;
          beginTurn(res.replacementTurnId, payload.replacementAction, res.status);
        } else {
          const current = useGameStore.getState().activeTurn;
          if (!current || current.turnId !== res.turnId) {
            hydratedTurnKeyRef.current = `${sessionId}:${res.turnId}`;
            beginTurn(res.turnId, null, res.status);
          } else {
            useGameStore.setState({
              activeTurn: {
                ...current,
                status: res.status,
              },
            });
          }
        }

        void queryClient.invalidateQueries({ queryKey: ["sessionState", sessionId] });
        void queryClient.invalidateQueries({ queryKey: ["evidence", sessionId] });
      } catch (err) {
        if (err instanceof ApiError && err.status === 409) {
          setInterruptError("该检定状态已更新，正在同步最新进度。");
          void resyncSession();
        } else {
          setInterruptError(
            err instanceof Error && err.message ? err.message : "提交检定决策未成功，请重试。",
          );
        }
      } finally {
        setIsResolvingInterrupt(false);
      }
    },
    [sessionId, pendingInterrupt, setPendingInterrupt, clearActiveTurn, beginTurn, queryClient, resyncSession],
  );

  const changeExperienceMode = useCallback(
    async (mode: ExperienceMode) => {
      setExperienceMode(mode);
      if (!sessionId) return;
      try {
        await vellumApi.updateExperienceMode(sessionId, mode);
        void queryClient.invalidateQueries({ queryKey: ["sessionState", sessionId] });
      } catch {
        // Local mode switch still applies immediately
      }
    },
    [sessionId, setExperienceMode, queryClient],
  );

  const resetToTurnZero = useCallback(() => {
    syncedModeSessionRef.current = null;
    hydratedTurnKeyRef.current = null;
    setActiveDrawer(null);
    setConnectionStatus("idle");
    setSessionId(null);
  }, [setActiveDrawer, setConnectionStatus, setSessionId]);

  return {
    sessionId,
    sessionState,
    isLoadingSession: sessionQuery.isLoading,
    isSessionError: sessionQuery.isError,
    sessionError: sessionQuery.error,
    refetchSessionState: sessionQuery.refetch,
    activeTurn,
    pendingInterrupt,
    isResolvingInterrupt,
    interruptError,
    startSessionFromTurn0,
    submitTurn,
    resolveInterrupt,
    changeExperienceMode,
    resetToTurnZero,
  };
}
