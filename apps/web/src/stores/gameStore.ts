import { create } from "zustand";
import type {
  DynamicViewDirective,
  ExperienceMode,
  PendingInterruptDTO,
  ServerStreamEvent,
  ThemeMode,
  TurnStatus,
} from "../api/types";

export interface ActiveTurnViewState {
  turnId: string;
  playerInput: string | null;
  status: TurnStatus;
  generationId: number;
  statusHint: string | null;
  narrationText: string;
  narrationComplete: boolean;
  typewriterSkipped: boolean;
  immediateDirectives: DynamicViewDirective[];
  pendingAfterNarration: DynamicViewDirective[];
  releasedAfterNarration: DynamicViewDirective[];
  errorMessage: string | null;
}

interface GameUiState {
  sessionId: string | null;
  theme: ThemeMode;
  experienceMode: ExperienceMode;
  lowSanEffectsEnabled: boolean;
  activeDrawer: "dossier" | "evidence" | "ooc" | null;
  connectionStatus: "idle" | "connecting" | "open" | "reconnecting" | "closed";
  activeTurn: ActiveTurnViewState | null;
  pendingInterrupt: PendingInterruptDTO | null;
  displayedDirectiveIds: Record<string, boolean>;
  newlyReleasedDirectiveIds: string[];

  setSessionId: (sessionId: string | null) => void;
  setTheme: (theme: ThemeMode) => void;
  toggleTheme: () => void;
  setExperienceMode: (mode: ExperienceMode) => void;
  setLowSanEffectsEnabled: (enabled: boolean) => void;
  setActiveDrawer: (drawer: "dossier" | "evidence" | "ooc" | null) => void;
  setConnectionStatus: (status: GameUiState["connectionStatus"]) => void;
  setPendingInterrupt: (interrupt: PendingInterruptDTO | null) => void;

  beginTurn: (turnId: string, playerInput: string | null, initialStatus?: TurnStatus) => void;
  hydrateTurnFromSnapshot: (
    turnId: string,
    playerInput: string | null,
    status: TurnStatus,
    generationId: number,
    narrationText: string,
    directives: DynamicViewDirective[],
    pendingInterrupt: PendingInterruptDTO | null,
  ) => void;
  applyStreamEvent: (event: ServerStreamEvent) => void;
  skipTypewriter: () => void;
  consumeNewlyReleasedDirectives: () => string[];
  clearActiveTurn: () => void;
}

const THEME_STORAGE_KEY = "vellum:theme";
const SESSION_STORAGE_KEY = "vellum:session_id";
const LOW_SAN_STORAGE_KEY = "vellum:low_san_fx";

function loadInitialTheme(): ThemeMode {
  if (typeof window === "undefined") return "dark";
  const saved = window.localStorage.getItem(THEME_STORAGE_KEY);
  return saved === "light" ? "light" : "dark";
}

function loadInitialSessionId(): string | null {
  if (typeof window === "undefined") return null;
  return window.localStorage.getItem(SESSION_STORAGE_KEY);
}

function loadInitialLowSanFx(): boolean {
  if (typeof window === "undefined") return true;
  const saved = window.localStorage.getItem(LOW_SAN_STORAGE_KEY);
  return saved === null ? true : saved === "true";
}

function upsertDirective(list: DynamicViewDirective[], incoming: DynamicViewDirective): DynamicViewDirective[] {
  const idx = list.findIndex((d) => d.directiveId === incoming.directiveId);
  if (idx === -1) return [...list, incoming];
  const next = [...list];
  next[idx] = incoming;
  return next;
}

function sortDirectivesBySeq(directives: DynamicViewDirective[]): DynamicViewDirective[] {
  return [...directives].sort(
    (a, b) => (a.seq ?? Number.MAX_SAFE_INTEGER) - (b.seq ?? Number.MAX_SAFE_INTEGER),
  );
}

function partitionDirectives(
  directives: DynamicViewDirective[],
  narrationFinished: boolean,
  displayedMap: Record<string, boolean>,
) {
  const immediate: DynamicViewDirective[] = [];
  const pending: DynamicViewDirective[] = [];
  const released: DynamicViewDirective[] = [];
  const nextDisplayed = { ...displayedMap };

  for (const d of sortDirectivesBySeq(directives)) {
    if (d.displayed) {
      nextDisplayed[d.directiveId] = true;
    }
    if (d.displayTiming === "immediate") {
      immediate.push(d);
    } else if (narrationFinished || d.displayed || nextDisplayed[d.directiveId]) {
      released.push(d);
    } else {
      pending.push(d);
    }
  }

  return { immediate, pending, released, nextDisplayed };
}

export const useGameStore = create<GameUiState>((set, get) => ({
  sessionId: loadInitialSessionId(),
  theme: loadInitialTheme(),
  experienceMode: "classic",
  lowSanEffectsEnabled: loadInitialLowSanFx(),
  activeDrawer: null,
  connectionStatus: "idle",
  activeTurn: null,
  pendingInterrupt: null,
  displayedDirectiveIds: {},
  newlyReleasedDirectiveIds: [],

  setSessionId: (sessionId) => {
    if (typeof window !== "undefined") {
      if (sessionId) window.localStorage.setItem(SESSION_STORAGE_KEY, sessionId);
      else window.localStorage.removeItem(SESSION_STORAGE_KEY);
    }
    set({ sessionId, activeTurn: null, pendingInterrupt: null });
  },

  setTheme: (theme) => {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(THEME_STORAGE_KEY, theme);
      document.documentElement.dataset.theme = theme;
    }
    set({ theme });
  },

  toggleTheme: () => {
    const next: ThemeMode = get().theme === "light" ? "dark" : "light";
    get().setTheme(next);
  },

  setExperienceMode: (experienceMode) => set({ experienceMode }),

  setLowSanEffectsEnabled: (lowSanEffectsEnabled) => {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(LOW_SAN_STORAGE_KEY, String(lowSanEffectsEnabled));
    }
    set({ lowSanEffectsEnabled });
  },

  setActiveDrawer: (activeDrawer) => set({ activeDrawer }),
  setConnectionStatus: (connectionStatus) => set({ connectionStatus }),
  setPendingInterrupt: (pendingInterrupt) => set({ pendingInterrupt }),

  beginTurn: (turnId, playerInput, initialStatus = "received") =>
    set({
      activeTurn: {
        turnId,
        playerInput,
        status: initialStatus,
        generationId: 1,
        statusHint: "正在整理当前局势……",
        narrationText: "",
        narrationComplete: false,
        typewriterSkipped: false,
        immediateDirectives: [],
        pendingAfterNarration: [],
        releasedAfterNarration: [],
        errorMessage: null,
      },
    }),

  hydrateTurnFromSnapshot: (
    turnId,
    playerInput,
    status,
    generationId,
    narrationText,
    directives,
    pendingInterrupt,
  ) =>
    set((state) => {
      const narrationFinished = status === "completed";
      const { immediate, pending, released, nextDisplayed } = partitionDirectives(
        directives,
        narrationFinished,
        state.displayedDirectiveIds,
      );
      return {
        pendingInterrupt,
        displayedDirectiveIds: nextDisplayed,
        activeTurn: {
          turnId,
          playerInput,
          status,
          generationId,
          statusHint: null,
          narrationText,
          narrationComplete: narrationFinished,
          typewriterSkipped: narrationFinished,
          immediateDirectives: immediate,
          pendingAfterNarration: pending,
          releasedAfterNarration: released,
          errorMessage: null,
        },
      };
    }),

  applyStreamEvent: (event) =>
    set((state) => {
      const current = state.activeTurn;

      switch (event.type) {
        case "status_hint": {
          if (!current || current.turnId !== event.turnId) return state;
          return {
            activeTurn: { ...current, statusHint: event.text },
          };
        }

        case "recovery_started": {
          if (!current || current.turnId !== event.turnId) return state;
          return {
            activeTurn: {
              ...current,
              status: "recovering",
              statusHint: "正在恢复本回合叙事……",
            },
          };
        }

        case "narration_reset": {
          if (!current || current.turnId !== event.turnId) return state;
          // Clear partial old generation text; keep kernel_owned pending directives rebound to new generation
          const reboundPending = current.pendingAfterNarration
            .filter((d) => d.source === "kernel_owned")
            .map((d) => ({ ...d, narrationGeneration: event.generationId }));

          return {
            activeTurn: {
              ...current,
              generationId: event.generationId,
              narrationText: "",
              narrationComplete: false,
              typewriterSkipped: false,
              pendingAfterNarration: reboundPending,
              status: "narrating",
              statusHint: null,
            },
          };
        }

        case "narration_delta": {
          if (!current || current.turnId !== event.turnId) return state;
          if (event.generationId < current.generationId) {
            return state; // Ignore stale generation chunks
          }
          return {
            activeTurn: {
              ...current,
              generationId: event.generationId,
              status: "narrating",
              statusHint: null,
              narrationText: current.narrationText + event.chunk,
            },
          };
        }

        case "narration_complete": {
          if (!current || current.turnId !== event.turnId) return state;
          if (event.generationId < current.generationId) {
            return state;
          }
          const toRelease = sortDirectivesBySeq(
            current.pendingAfterNarration.filter(
              (d) => d.narrationGeneration === event.generationId || d.source === "kernel_owned",
            ),
          );
          const nextDisplayed = { ...state.displayedDirectiveIds };
          const newlyReleased: string[] = [];
          let releasedList = current.releasedAfterNarration;

          for (const d of toRelease) {
            releasedList = upsertDirective(releasedList, { ...d, displayed: true });
            if (!nextDisplayed[d.directiveId]) {
              nextDisplayed[d.directiveId] = true;
              newlyReleased.push(d.directiveId);
            }
          }

          return {
            displayedDirectiveIds: nextDisplayed,
            newlyReleasedDirectiveIds: [...state.newlyReleasedDirectiveIds, ...newlyReleased],
            activeTurn: {
              ...current,
              narrationComplete: true,
              pendingAfterNarration: [],
              releasedAfterNarration: releasedList,
            },
          };
        }

        case "ui_directive": {
          const directive = event.directive;
          if (!current || current.turnId !== directive.turnId) return state;
          if (
            directive.source === "narrative_owned" &&
            directive.narrationGeneration < current.generationId
          ) {
            return state;
          }

          if (directive.displayTiming === "immediate") {
            return {
              activeTurn: {
                ...current,
                immediateDirectives: upsertDirective(current.immediateDirectives, directive),
              },
            };
          }

          // after_narration directive: hold until narration_complete (or already displayed / skipped)
          const canReleaseNow =
            current.narrationComplete ||
            Boolean(directive.displayed) ||
            Boolean(state.displayedDirectiveIds[directive.directiveId]);

          if (canReleaseNow) {
            const nextDisplayed = { ...state.displayedDirectiveIds };
            const newlyReleased = [...state.newlyReleasedDirectiveIds];
            if (!nextDisplayed[directive.directiveId]) {
              nextDisplayed[directive.directiveId] = true;
              newlyReleased.push(directive.directiveId);
            }
            return {
              displayedDirectiveIds: nextDisplayed,
              newlyReleasedDirectiveIds: newlyReleased,
              activeTurn: {
                ...current,
                releasedAfterNarration: upsertDirective(current.releasedAfterNarration, {
                  ...directive,
                  displayed: true,
                }),
              },
            };
          }

          return {
            activeTurn: {
              ...current,
              pendingAfterNarration: upsertDirective(current.pendingAfterNarration, directive),
            },
          };
        }

        case "interrupt_required": {
          return {
            pendingInterrupt: event.interrupt,
            activeTurn:
              current && current.turnId === event.interrupt.turnId
                ? { ...current, status: event.interrupt.status, statusHint: null }
                : current,
          };
        }

        case "stream_reset": {
          const snap = event.snapshot;
          const narrationFinished = snap.status === "completed";
          const { immediate, pending, released, nextDisplayed } = partitionDirectives(
            snap.directives,
            narrationFinished,
            state.displayedDirectiveIds,
          );
          return {
            pendingInterrupt: snap.pendingInterrupt ?? state.pendingInterrupt,
            displayedDirectiveIds: nextDisplayed,
            activeTurn: {
              turnId: snap.turnId,
              playerInput: current?.turnId === snap.turnId ? current.playerInput : null,
              status: snap.status,
              generationId: snap.generationId,
              statusHint: null,
              narrationText: snap.narrationSnapshot,
              narrationComplete: narrationFinished,
              typewriterSkipped: narrationFinished,
              immediateDirectives: immediate,
              pendingAfterNarration: pending,
              releasedAfterNarration: released,
              errorMessage: null,
            },
          };
        }

        case "turn_complete": {
          if (!current || current.turnId !== event.turnId) return state;
          return {
            activeTurn: {
              ...current,
              status: "completed",
              statusHint: null,
              narrationComplete: true,
            },
          };
        }

        case "turn_error": {
          if (!current || current.turnId !== event.turnId) return state;
          // Retain pending kernel_owned directives without surfacing them prematurely
          return {
            activeTurn: {
              ...current,
              status: "failed",
              statusHint: null,
              errorMessage: event.message,
            },
          };
        }

        default:
          return state;
      }
    }),

  skipTypewriter: () =>
    set((state) => {
      const current = state.activeTurn;
      if (!current) return state;
      // Skipping typewriter only skips visual reveal; if narration is already complete, flush pending directives
      if (!current.narrationComplete) {
        return { activeTurn: { ...current, typewriterSkipped: true } };
      }
      const nextDisplayed = { ...state.displayedDirectiveIds };
      const newlyReleased = [...state.newlyReleasedDirectiveIds];
      let releasedList = current.releasedAfterNarration;
      for (const d of current.pendingAfterNarration) {
        releasedList = upsertDirective(releasedList, { ...d, displayed: true });
        if (!nextDisplayed[d.directiveId]) {
          nextDisplayed[d.directiveId] = true;
          newlyReleased.push(d.directiveId);
        }
      }
      return {
        displayedDirectiveIds: nextDisplayed,
        newlyReleasedDirectiveIds: newlyReleased,
        activeTurn: {
          ...current,
          typewriterSkipped: true,
          pendingAfterNarration: [],
          releasedAfterNarration: releasedList,
        },
      };
    }),

  consumeNewlyReleasedDirectives: () => {
    const ids = get().newlyReleasedDirectiveIds;
    if (ids.length > 0) {
      set({ newlyReleasedDirectiveIds: [] });
    }
    return ids;
  },

  clearActiveTurn: () => set({ activeTurn: null }),
}));
