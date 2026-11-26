import type {
  ApiErrorEnvelope,
  CaseDebriefDTO,
  ClientVisibleStateDTO,
  CreateEvidenceNoteRequest,
  CreateSessionRequest,
  CreateSessionResponse,
  CreateTurnRequest,
  CreateTurnResponse,
  EvidenceBoardDTO,
  EvidenceNoteDTO,
  ExperienceMode,
  InvestigatorPresetDTO,
  OocRequest,
  OocResponse,
  ResolveInterruptRequest,
  ResolveInterruptResponse,
  SessionMetaDTO,
  TurnSnapshotDTO,
} from "./types";

const API_BASE = import.meta.env.VITE_API_BASE ?? "";

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly details?: Record<string, unknown>;

  constructor(status: number, code: string, message: string, details?: Record<string, unknown>) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
    this.details = details;
  }
}

export function createIdempotencyKey(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `idem-${Date.now()}-${Math.random().toString(36).slice(2, 11)}`;
}

async function requestJson<T>(
  path: string,
  options: {
    method?: "GET" | "POST" | "PATCH";
    body?: unknown;
    idempotencyKey?: string;
    signal?: AbortSignal;
  } = {},
): Promise<T> {
  const headers: Record<string, string> = {
    Accept: "application/json",
  };
  if (options.body !== undefined) {
    headers["Content-Type"] = "application/json";
  }
  if (options.idempotencyKey) {
    headers["Idempotency-Key"] = options.idempotencyKey;
  }

  const res = await fetch(`${API_BASE}${path}`, {
    method: options.method ?? "GET",
    headers,
    body: options.body !== undefined ? JSON.stringify(options.body) : undefined,
    signal: options.signal,
  });

  if (!res.ok) {
    let parsed: ApiErrorEnvelope | null = null;
    try {
      parsed = (await res.json()) as ApiErrorEnvelope;
    } catch {
      parsed = null;
    }
    throw new ApiError(
      res.status,
      parsed?.error?.code ?? `HTTP_${res.status}`,
      parsed?.error?.message ?? res.statusText,
      parsed?.error?.details,
    );
  }

  if (res.status === 204) {
    return undefined as T;
  }
  return (await res.json()) as T;
}

export const vellumApi = {
  getHealth: () => requestJson<{ status: string; llmMode: string }>("/api/health"),

  listInvestigators: (moduleId: string) =>
    requestJson<{ moduleId: string; moduleTitle: string; investigators: InvestigatorPresetDTO[] }>(
      `/api/modules/${encodeURIComponent(moduleId)}/investigators`,
    ),

  createSession: (payload: CreateSessionRequest, idempotencyKey = createIdempotencyKey()) =>
    requestJson<CreateSessionResponse>("/api/sessions", {
      method: "POST",
      body: payload,
      idempotencyKey,
    }),

  getSession: (sessionId: string) =>
    requestJson<SessionMetaDTO>(`/api/sessions/${encodeURIComponent(sessionId)}`),

  getSessionState: (sessionId: string) =>
    requestJson<ClientVisibleStateDTO>(`/api/sessions/${encodeURIComponent(sessionId)}/state`),

  updateExperienceMode: (sessionId: string, experienceMode: ExperienceMode) =>
    requestJson<SessionMetaDTO>(`/api/sessions/${encodeURIComponent(sessionId)}/mode`, {
      method: "PATCH",
      body: { experienceMode },
    }),

  createTurn: (
    sessionId: string,
    payload: CreateTurnRequest,
    idempotencyKey = createIdempotencyKey(),
  ) =>
    requestJson<CreateTurnResponse>(`/api/sessions/${encodeURIComponent(sessionId)}/turns`, {
      method: "POST",
      body: payload,
      idempotencyKey,
    }),

  getTurn: (sessionId: string, turnId: string) =>
    requestJson<TurnSnapshotDTO>(
      `/api/sessions/${encodeURIComponent(sessionId)}/turns/${encodeURIComponent(turnId)}`,
    ),

  resolveInterrupt: (
    sessionId: string,
    interruptId: string,
    payload: ResolveInterruptRequest,
    idempotencyKey = createIdempotencyKey(),
  ) =>
    requestJson<ResolveInterruptResponse>(
      `/api/sessions/${encodeURIComponent(sessionId)}/interrupts/${encodeURIComponent(interruptId)}/resolve`,
      {
        method: "POST",
        body: payload,
        idempotencyKey,
      },
    ),

  askOoc: (sessionId: string, payload: OocRequest) =>
    requestJson<OocResponse>(`/api/sessions/${encodeURIComponent(sessionId)}/ooc`, {
      method: "POST",
      body: payload,
    }),

  getEvidence: (sessionId: string) =>
    requestJson<EvidenceBoardDTO>(`/api/sessions/${encodeURIComponent(sessionId)}/evidence`),

  createEvidenceNote: (
    sessionId: string,
    payload: CreateEvidenceNoteRequest,
    idempotencyKey = createIdempotencyKey(),
  ) =>
    requestJson<EvidenceNoteDTO>(
      `/api/sessions/${encodeURIComponent(sessionId)}/evidence/notes`,
      {
        method: "POST",
        body: payload,
        idempotencyKey,
      },
    ),

  ackDirective: (sessionId: string, directiveId: string) =>
    requestJson<{ ok: boolean }>(
      `/api/sessions/${encodeURIComponent(sessionId)}/directives/${encodeURIComponent(directiveId)}/ack`,
      { method: "POST" },
    ),

  getDebrief: (sessionId: string) =>
    requestJson<CaseDebriefDTO>(`/api/sessions/${encodeURIComponent(sessionId)}/debrief`),
};
