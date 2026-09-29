/**
 * Log stream events crossing the Tauri boundary over a Channel.
 *
 * Mirrors `LogStreamEvent` in `app/src-tauri/src/session/log_streams.rs`; the shared fixture at
 * `app/tests/fixtures/logs-ipc.json` keeps both sides in sync.
 */
import type { Tone } from "@/components/kit/badges";
import { exactRecord } from "@/types/inventory";

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

const isCount = (value: unknown): value is number => typeof value === "number" && Number.isInteger(value) && value >= 0;

function lineFromIpc(value: unknown): LogLine {
    const data = exactRecord(value, ["seq", "ts", "stream", "taskId", "nodeId", "text", "truncated"]);
    if (
        !isCount(data.seq) ||
        !isCount(data.ts) ||
        (data.stream !== "stdout" && data.stream !== "stderr") ||
        typeof data.taskId !== "string" ||
        typeof data.nodeId !== "string" ||
        typeof data.text !== "string" ||
        typeof data.truncated !== "boolean"
    )
        throw "invalid_response";
    return data as unknown as LogLine;
}

export function logStreamEventFromIpc(value: unknown): LogStreamEvent {
    const type = (value as { type?: unknown } | null)?.type;
    switch (type) {
        case "lines": {
            const data = exactRecord(value, ["type", "lines"]);
            if (!Array.isArray(data.lines) || data.lines.length > 1_000) throw "invalid_response";
            return { type, lines: data.lines.map(lineFromIpc) };
        }
        case "gap": {
            const data = exactRecord(value, ["type", "dropped"]);
            if (!isCount(data.dropped)) throw "invalid_response";
            return { type, dropped: data.dropped };
        }
        case "reconnecting": {
            const data = exactRecord(value, ["type", "attempt", "nextRetryMs"]);
            if (!isCount(data.attempt) || !isCount(data.nextRetryMs)) throw "invalid_response";
            return { type, attempt: data.attempt, nextRetryMs: data.nextRetryMs };
        }
        case "reconnected":
            exactRecord(value, ["type"]);
            return { type };
        case "ended": {
            const data = exactRecord(value, ["type", "reason", "code", "message"]);
            if ((data.reason !== "service_removed" && data.reason !== "error") || typeof data.code !== "string" || typeof data.message !== "string")
                throw "invalid_response";
            return { type, reason: data.reason, code: data.code, message: data.message };
        }
        default:
            throw "invalid_response";
    }
}

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
