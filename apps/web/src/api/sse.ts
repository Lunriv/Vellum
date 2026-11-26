import type { ServerStreamEvent } from "./types";

const API_BASE = import.meta.env.VITE_API_BASE ?? "";

export interface TurnStreamCallbacks {
  onEvent: (event: ServerStreamEvent, seq: number | null) => void;
  onStatusChange?: (status: "connecting" | "open" | "reconnecting" | "closed") => void;
  onError?: (error: Error) => void;
}

export interface TurnStreamHandle {
  close: () => void;
  getLastEventId: () => string | null;
}

/**
 * Fetch-backed SSE subscriber so we can send Last-Event-ID deterministically on reconnect
 * without depending on native EventSource's opaque header limitations.
 */
export function subscribeTurnEvents(
  sessionId: string,
  turnId: string,
  callbacks: TurnStreamCallbacks,
  initialLastEventId: string | null = null,
): TurnStreamHandle {
  let closed = false;
  let lastEventId = initialLastEventId;
  let retryAttempt = 0;
  let controller: AbortController | null = null;
  let retryTimer: ReturnType<typeof setTimeout> | null = null;

  const url = `${API_BASE}/api/sessions/${encodeURIComponent(sessionId)}/turns/${encodeURIComponent(turnId)}/events`;

  const scheduleReconnect = () => {
    if (closed) return;
    retryAttempt += 1;
    const delayMs = Math.min(4000, 350 * 2 ** Math.min(retryAttempt - 1, 4));
    callbacks.onStatusChange?.("reconnecting");
    retryTimer = setTimeout(() => {
      void connect();
    }, delayMs);
  };

  const connect = async () => {
    if (closed) return;
    controller = new AbortController();
    callbacks.onStatusChange?.(retryAttempt === 0 ? "connecting" : "reconnecting");

    const headers: Record<string, string> = {
      Accept: "text/event-stream",
    };
    if (lastEventId) {
      headers["Last-Event-ID"] = lastEventId;
    }

    try {
      const response = await fetch(url, {
        method: "GET",
        headers,
        signal: controller.signal,
      });

      if (!response.ok || !response.body) {
        throw new Error(`SSE stream failed with status ${response.status}`);
      }

      retryAttempt = 0;
      callbacks.onStatusChange?.("open");

      const reader = response.body.getReader();
      const decoder = new TextDecoder("utf-8");
      let buffer = "";
      let reachedTerminal = false;

      const processBlock = (rawBlock: string) => {
        const lines = rawBlock.split(/\r?\n/);
        let frameId: string | null = null;
        const dataLines: string[] = [];

        for (const line of lines) {
          if (line.startsWith(":") || line.length === 0) continue;
          if (line.startsWith("id:")) {
            frameId = line.slice(3).trim();
          } else if (line.startsWith("data:")) {
            dataLines.push(line.slice(5).trimStart());
          }
        }

        if (dataLines.length === 0) return;
        if (frameId) {
          lastEventId = frameId;
        }

        const parsed = JSON.parse(dataLines.join("\n")) as ServerStreamEvent;
        const numericSeq = frameId && /^\d+$/.test(frameId) ? Number(frameId) : null;
        if (parsed.type === "stream_reset") {
          lastEventId = String(parsed.snapshot.lastSeq);
        }
        callbacks.onEvent(parsed, numericSeq);

        if (parsed.type === "turn_complete" || parsed.type === "turn_error") {
          reachedTerminal = true;
        }
      };

      while (!closed) {
        const { done, value } = await reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });

        let splitIdx = buffer.indexOf("\n\n");
        while (splitIdx !== -1) {
          const block = buffer.slice(0, splitIdx);
          buffer = buffer.slice(splitIdx + 2);
          if (block.trim().length > 0) {
            processBlock(block);
          }
          splitIdx = buffer.indexOf("\n\n");
        }
      }

      if (reachedTerminal || closed) {
        closed = true;
        callbacks.onStatusChange?.("closed");
      } else {
        scheduleReconnect();
      }
    } catch (err) {
      if (closed || (err instanceof DOMException && err.name === "AbortError")) {
        callbacks.onStatusChange?.("closed");
        return;
      }
      callbacks.onError?.(err instanceof Error ? err : new Error(String(err)));
      scheduleReconnect();
    }
  };

  void connect();

  return {
    close: () => {
      closed = true;
      if (retryTimer) clearTimeout(retryTimer);
      controller?.abort();
      callbacks.onStatusChange?.("closed");
    },
    getLastEventId: () => lastEventId,
  };
}
