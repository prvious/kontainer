/**
 * Log stream events crossing the Tauri boundary over a Channel.
 *
 * Mirrors `LogStreamEvent` in `app/src-tauri/src/session/log_streams.rs`; the shared fixture at
 * `app/tests/fixtures/logs-ipc.json` keeps both sides in sync.
 */
import type { Tone } from "@/components/kit/badges";

export interface LogLine {
    /** Lines delivered by this stream, counted across reconnects. */
    seq: number;
    /** Epoch milliseconds. */
    ts: number;
    stream: "stdout" | "stderr";
    taskId: string;
    nodeId: string;
    text: string;
    truncated: boolean;
}

export type LogStreamEvent =
    | { type: "lines"; lines: LogLine[] }
    | { type: "gap"; dropped: number }
    | { type: "reconnecting"; attempt: number; nextRetryMs: number }
    | { type: "reconnected" }
    | { type: "ended"; reason: "service_removed" | "error"; code: string; message: string };

/** History an open asks for by default, and the most LOAD PREVIOUS HOUR asks for. */
export const DEFAULT_LOG_TAIL = 500;
export const MAX_LOG_TAIL = 5_000;
/** The buffer keeps this many lines when the scrollback setting is unset. */
export const DEFAULT_LOG_SCROLLBACK = 10_000;

export interface LogLevel {
    label: "FATAL" | "ERROR" | "WARN" | "INFO" | "DEBUG" | "TRACE" | "ERR";
    tone: Tone;
}

const LEVEL = /\b(FATAL|ERROR|WARN(?:ING)?|INFO|DEBUG|TRACE)\b/i;
const LEVEL_TONES: Record<Exclude<LogLevel["label"], "ERR">, Tone> = {
    FATAL: "destructive",
    ERROR: "destructive",
    WARN: "warning",
    INFO: "neutral",
    DEBUG: "neutral",
    TRACE: "neutral",
};

/** The level named in the line; stderr lines naming none are `ERR`; stdout ones get no badge. */
export function logLevel(line: Pick<LogLine, "text" | "stream">): LogLevel | null {
    const match = LEVEL.exec(line.text);
    if (match) {
        const word = match[1].toUpperCase();
        const label = (word === "WARNING" ? "WARN" : word) as Exclude<LogLevel["label"], "ERR">;
        return { label, tone: LEVEL_TONES[label] };
    }
    return line.stream === "stderr" ? { label: "ERR", tone: "destructive" } : null;
}

/**
 * The filter box: a case-insensitive substring, or a regular expression when wrapped in `/`.
 * An empty query matches everything; an invalid expression is searched for literally.
 */
export function lineMatcher(query: string): (text: string) => boolean {
    const trimmed = query.trim();
    if (trimmed === "") return () => true;
    if (trimmed.length > 2 && trimmed.startsWith("/") && trimmed.endsWith("/")) {
        try {
            const pattern = new RegExp(trimmed.slice(1, -1), "i");
            return (text) => pattern.test(text);
        } catch {
            // Fall through to a literal search.
        }
    }
    const needle = trimmed.toLowerCase();
    return (text) => text.toLowerCase().includes(needle);
}
