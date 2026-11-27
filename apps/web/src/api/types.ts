/**
 * Frozen Frontend/Backend Contract Types for Vellum.
 * Synchronized with docs/ARCHITECTURE_PLAN.md Section 4 & docs/ARCHITECTURE.md v0.3.1.
 */

export type ExperienceMode = "guided" | "classic" | "immersive";
export type ThemeMode = "light" | "dark";
export type SessionStatus = "active" | "ended";

export type TurnStatus =
  | "received"
  | "routing"
  | "proposal_pending"
  | "awaiting_roll"
  | "awaiting_recovery"
  | "awaiting_push_description"
  | "awaiting_push_consequence_ack"
  | "awaiting_push_roll"
  | "committing"
  | "narrating"
  | "narration_pending"
  | "recovering"
  | "completed"
  | "cancelled"
  | "failed"
  | "interrupted_before_commit";

export type CheckDifficulty = "regular" | "hard" | "extreme";
export type RollResultLevel =
  | "critical_success"
  | "extreme_success"
  | "hard_success"
  | "regular_success"
  | "failure"
  | "fumble";

export type DirectiveSource = "kernel_owned" | "narrative_owned";
export type DisplayTiming = "immediate" | "after_narration";
export type DirectiveType =
  | "inline_dice"
  | "atmosphere_shift"
  | "diegetic_handout"
  | "evidence_unlocked"
  | "clock_pulse"
  | "character_state_delta"
  | "narrative_card";

export interface ApiErrorEnvelope {
  error: {
    code: string;
    message: string;
    details?: Record<string, unknown>;
  };
}

export interface InvestigatorPresetDTO {
  investigatorId: string;
  name: string;
  occupation: string;
  tagline: string;
  biography: string;
  motivation: string;
  manorConnection: string;
  hpMax: number;
  mpMax: number;
  sanStart: number;
  luckStart: number;
  strengths: Array<{ skill: string; label: string; rating: number }>;
  startingItems: Array<{ itemId: string; name: string; description: string }>;
}

export interface InvestigatorStateDTO {
  investigatorId: string;
  name: string;
  occupation: string;
  hp: number;
  hpMax: number;
  mp: number;
  mpMax: number;
  san: number;
  sanMax: number;
  luck: number;
  luckMax: number;
  characteristics: Record<string, number>;
  skills: Record<string, number>;
  inventory: Array<{ itemId: string; name: string; description: string; quantity: number }>;
  conditions: string[];
}

export interface SceneExitDTO {
  targetSceneId: string;
  label: string;
  accessible: boolean;
  blockedReason?: string;
}

export interface SceneEntityDTO {
  entityId: string;
  name: string;
  role: string;
  kind: "npc" | "object";
  summary: string;
}

export interface VisibleSceneDTO {
  sceneId: string;
  title: string;
  atmosphere: "rain_manor" | "candlelight" | "fog" | "san_strain";
  description: string;
  tags: string[];
  visitedSceneIds?: string[];
  exits: SceneExitDTO[];
  presentEntities: SceneEntityDTO[];
}

export interface MapRoomDTO {
  sceneId: string;
  title: string;
  atmosphere: "rain_manor" | "candlelight" | "fog" | "san_strain";
  visited: boolean;
  isCurrent: boolean;
  accessibleFromCurrent: boolean;
  exitLabelFromCurrent?: string | null;
  knownNpcs: Array<{ entityId: string; name: string }>;
}

export interface VisibleClockDTO {
  clockId: string;
  name: string;
  current: number;
  max: number;
  stageLabel: string;
  lastReason?: string;
}

export interface ClueDTO {
  clueId: string;
  title: string;
  summary: string;
  sourceLabel: string;
  discoveredAtTurnId: string;
  isConclusion?: boolean;
}

export interface HandoutDTO {
  handoutId: string;
  kind: "letter" | "diary" | "newspaper" | "telegram" | "report" | "note";
  title: string;
  canonical: boolean;
  clueId?: string;
  sourceFacts: string[];
  presentationText: string;
  unlockedAtTurnId: string;
}

export interface RollAttemptDTO {
  checkId: string;
  attemptIndex: number;
  pushCount: number;
  skill: string;
  skillLabel?: string;
  difficulty: CheckDifficulty;
  targetNumber: number;
  bonusPenaltyDice: number;
  rollValue: number;
  resultLevel: RollResultLevel;
  passed: boolean;
  consequenceRef?: string | null;
}

export interface RecoveryEligibilityDTO {
  canAcceptFailure: boolean;
  canSpendLuck: boolean;
  luckCostNeeded?: number | null;
  canPushRoll: boolean;
  pushIneligibleReason?: string | null;
}

export interface PendingInterruptDTO {
  interruptId: string;
  sessionId: string;
  turnId: string;
  status:
    | "awaiting_roll"
    | "awaiting_recovery"
    | "awaiting_push_description"
    | "awaiting_push_consequence_ack"
    | "awaiting_push_roll";
  actionSummary: string;
  skill: string;
  skillLabel: string;
  difficulty: CheckDifficulty;
  targetNumber: number;
  bonusPenaltyDice: number;
  stakesHint: string;
  firstRoll?: RollAttemptDTO | null;
  recovery?: RecoveryEligibilityDTO | null;
  pushDescription?: string | null;
  pushConsequence?: string | null;
}

export interface DynamicViewDirective {
  schemaVersion: number;
  directiveId: string;
  sessionId: string;
  turnId: string;
  narrationGeneration: number;
  source: DirectiveSource;
  directiveType: DirectiveType;
  displayTiming: DisplayTiming;
  displayed?: boolean;
  seq?: number;
  payload: Record<string, unknown>;
}

export interface TranscriptItemDTO {
  entryId: string;
  turnId: string;
  turnIndex: number;
  sceneId: string;
  playerInput?: string | null;
  narration: string;
  narrationGeneration: number;
  directives: DynamicViewDirective[];
  createdAt: string;
}

export interface SessionMetaDTO {
  sessionId: string;
  moduleId: string;
  moduleVersion: string;
  investigatorId: string;
  experienceMode: ExperienceMode;
  status: SessionStatus;
  sessionVersion: number;
  activeTurnId: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ClientVisibleStateDTO {
  session: SessionMetaDTO;
  investigator: InvestigatorStateDTO;
  currentScene: VisibleSceneDTO;
  mapRooms?: MapRoomDTO[];
  visibleClocks: VisibleClockDTO[];
  clues: ClueDTO[];
  handouts: HandoutDTO[];
  pendingInterrupt: PendingInterruptDTO | null;
  transcript: TranscriptItemDTO[];
  guidedPrompts?: string[];
}

export interface ClientVisibleStatePatch {
  sessionVersion?: number;
  sessionStatus?: SessionStatus;
  investigator?: Partial<InvestigatorStateDTO>;
  currentScene?: VisibleSceneDTO;
  mapRooms?: MapRoomDTO[];
  visibleClocks?: VisibleClockDTO[];
  addedClues?: ClueDTO[];
  addedHandouts?: HandoutDTO[];
  pendingInterrupt?: PendingInterruptDTO | null;
  guidedPrompts?: string[];
}

export interface CreateSessionRequest {
  moduleId: string;
  investigatorId: string;
  experienceMode: ExperienceMode;
}

export interface CreateSessionResponse {
  session: SessionMetaDTO;
  turn0Id: string;
  streamUrl: string;
}

export interface CreateTurnRequest {
  input: string;
}

export interface CreateTurnResponse {
  turnId: string;
  turnIndex: number;
  status: TurnStatus;
  streamUrl: string;
}

export interface TurnSnapshotDTO {
  turnId: string;
  sessionId: string;
  turnIndex: number;
  status: TurnStatus;
  playerInput?: string | null;
  narrationGeneration: number;
  narrationText: string;
  pendingInterrupt: PendingInterruptDTO | null;
  directives: DynamicViewDirective[];
}

export interface TurnStreamSnapshot {
  turnId: string;
  generationId: number;
  status: TurnStatus;
  lastSeq: number;
  narrationSnapshot: string;
  pendingInterrupt?: PendingInterruptDTO | null;
  directives: DynamicViewDirective[];
}

export type ResolveInterruptRequest =
  | { action: "cancel" }
  | { action: "revise_action"; replacementAction: string }
  | { action: "confirm_roll" }
  | { action: "accept_failure" }
  | { action: "spend_luck" }
  | { action: "push_roll" }
  | { action: "submit_push_description"; description: string }
  | { action: "confirm_push_consequence" }
  | { action: "accept_initial_failure" };

export interface ResolveInterruptResponse {
  turnId: string;
  replacementTurnId?: string | null;
  status: TurnStatus;
  pendingInterrupt: PendingInterruptDTO | null;
  roll?: RollAttemptDTO | null;
  streamUrl?: string | null;
}

export interface OocRequest {
  question: string;
}

export interface OocMessageDTO {
  id: string;
  role: "player" | "assistant";
  content: string;
  createdAt: string;
}

export interface OocResponse {
  answer: string;
  history: OocMessageDTO[];
}

export interface EvidenceNoteDTO {
  noteId: string;
  noteKind: "note" | "link";
  title?: string;
  content?: string;
  fromClueId?: string;
  toClueId?: string;
  label?: string;
  createdAt: string;
}

export interface EvidenceBoardDTO {
  clues: ClueDTO[];
  handouts: HandoutDTO[];
  notes: EvidenceNoteDTO[];
}

export interface CreateEvidenceNoteRequest {
  noteKind: "note" | "link";
  title?: string;
  content?: string;
  fromClueId?: string;
  toClueId?: string;
  label?: string;
}

export interface CaseDebriefDTO {
  sessionId: string;
  moduleTitle: string;
  spoilerWarning: true;
  endingTitle: string;
  endingSummary: string;
  trueChronology: Array<{ title: string; detail: string; sourceEventIds: string[] }>;
  npcMotives: Array<{
    entityId: string;
    name: string;
    motive: string;
    secretStages: Array<{ tier: string; unlocked: boolean; truth: string }>;
  }>;
  missedClues: Array<{ clueId: string; title: string; summary: string }>;
  finalClocks: Array<{
    clockId: string;
    name: string;
    visibility: "player_visible" | "keeper_hidden";
    current: number;
    max: number;
    stageLabel: string;
  }>;
  playerImpact: Array<{ choice: string; consequence: string; sourceEventIds: string[] }>;
  alternativeEndings: Array<{ title: string; conditionSummary: string; achieved: boolean }>;
}

export type ServerStreamEvent =
  | { type: "status_hint"; text: string; turnId: string }
  | { type: "narration_delta"; chunk: string; turnId: string; generationId: number }
  | { type: "narration_reset"; turnId: string; generationId: number; clearPartial: true }
  | { type: "narration_complete"; turnId: string; generationId: number }
  | { type: "ui_directive"; directive: DynamicViewDirective }
  | { type: "interrupt_required"; interrupt: PendingInterruptDTO }
  | { type: "state_delta"; patch: ClientVisibleStatePatch }
  | { type: "stream_reset"; turnId: string; snapshot: TurnStreamSnapshot }
  | { type: "recovery_started"; turnId: string; generationId: number }
  | { type: "turn_complete"; turnId: string }
  | { type: "turn_error"; turnId: string; code: string; message: string };
