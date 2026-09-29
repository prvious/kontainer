import type { LogLine, LogStreamEvent } from "@/types/logs";

export type LogRow = { kind: "line"; line: LogLine; key: string } | { kind: "gap"; dropped: number; key: string };
export type LogStatus = "connecting" | "live" | "reconnecting" | "ended";

export interface LogBufferSnapshot {
    rows: LogRow[];
    /** Lines received since the tab opened, across restarts; the follow pill counts from it. */
    received: number;
    firstTs: number | null;
    lastTs: number | null;
    status: LogStatus;
    /** While reconnecting: which attempt, and when the next one starts (epoch ms). */
    reconnect: { attempt: number; retryAt: number } | null;
    /** When the stream dropped, for the reconnecting alert. */
    droppedAt: number | null;
    ended: { reason: "service_removed" | "error"; code: string; message: string } | null;
}

const EMPTY: LogBufferSnapshot = {
    rows: [],
    received: 0,
    firstTs: null,
    lastTs: null,
    status: "connecting",
    reconnect: null,
    droppedAt: null,
    ended: null,
};

/**
 * One tab's lines, capped at the scrollback, fed by its stream's Channel. It is an external store:
 * the viewer reads it with `useSyncExternalStore`, and every change is a new snapshot.
 */
export class LogBuffer {
    #snapshot: LogBufferSnapshot = EMPTY;
    #listeners = new Set<() => void>();
    #capacity: number;
    #gaps = 0;
    /** Each stream this tab opens numbers its lines from 1, so keys carry the stream too. */
    #generation = 0;

    constructor(capacity: number) {
        this.#capacity = Math.max(1, capacity);
    }

    subscribe = (listener: () => void) => {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    };

    getSnapshot = () => this.#snapshot;

    #set(next: Partial<LogBufferSnapshot>) {
        this.#snapshot = { ...this.#snapshot, ...next };
        for (const listener of this.#listeners) listener();
    }

    /** The stream was accepted; it is live until it says otherwise. */
    opened() {
        if (this.#snapshot.status === "connecting") this.#set({ status: "live" });
    }

    /** A new stream replaces the old one: `keepLines` for RESUME, not for LOAD PREVIOUS HOUR. */
    restart(keepLines: boolean) {
        this.#generation += 1;
        this.#set(keepLines ? { status: "connecting", reconnect: null, droppedAt: null, ended: null } : { ...EMPTY, received: this.#snapshot.received });
    }

    apply(event: LogStreamEvent, receivedAt: number) {
        const current = this.#snapshot;
        switch (event.type) {
            case "lines": {
                if (event.lines.length === 0) return;
                const generation = this.#generation;
                const rows = current.rows.concat(event.lines.map((line) => ({ kind: "line" as const, line, key: `${generation}:${line.seq}` })));
                const kept = rows.length > this.#capacity ? rows.slice(rows.length - this.#capacity) : rows;
                const first = kept.find((row) => row.kind === "line");
                const last = event.lines[event.lines.length - 1];
                this.#set({
                    rows: kept,
                    received: current.received + event.lines.length,
                    firstTs: first?.kind === "line" ? first.line.ts : null,
                    lastTs: last.ts,
                    status: current.status === "ended" ? "ended" : "live",
                });
                return;
            }
            case "gap":
                this.#gaps += 1;
                this.#set({ rows: current.rows.concat({ kind: "gap", dropped: event.dropped, key: `gap-${this.#gaps}` }) });
                return;
            case "reconnecting":
                this.#set({
                    status: "reconnecting",
                    reconnect: { attempt: event.attempt, retryAt: receivedAt + event.nextRetryMs },
                    droppedAt: current.droppedAt ?? receivedAt,
                });
                return;
            case "reconnected":
                this.#set({ status: "live", reconnect: null, droppedAt: null });
                return;
            case "ended":
                this.#set({ status: "ended", reconnect: null, ended: { reason: event.reason, code: event.code, message: event.message } });
                return;
        }
    }
}
