/**
 * Exec session events crossing the Tauri boundary over a Channel.
 *
 * Mirrors `ExecStreamEvent` in `app/src-tauri/src/session/exec_sessions.rs`; the shared fixture
 * at `app/tests/fixtures/exec-ipc.json` keeps both sides in sync.
 */
import { exactRecord } from "@/types/inventory";

export type ExecStreamEvent =
    | { type: "started"; shell: string }
    /** Base64 of the raw bytes the terminal wrote. */
    | { type: "output"; data: string }
    | { type: "exit"; code: number | null; reason: string }
    | { type: "error"; code: string; message: string }
    | { type: "closed"; reason: string };

/** Exec sessions one sign-in may hold at once (ADR 0026). */
export const MAX_EXEC_SESSIONS = 4;
/** The size a session opens at before its terminal has measured itself. */
export const INITIAL_EXEC_SIZE = { cols: 120, rows: 32 } as const;

const BASE64 = /^[A-Za-z0-9+/]*={0,2}$/;
const isText = (value: unknown, limit: number): value is string => typeof value === "string" && value.length <= limit;

export function execStreamEventFromIpc(value: unknown): ExecStreamEvent {
    const type = (value as { type?: unknown } | null)?.type;
    switch (type) {
        case "started": {
            const data = exactRecord(value, ["type", "shell"]);
            if (!isText(data.shell, 256)) throw "invalid_response";
            return { type, shell: data.shell };
        }
        case "output": {
            const data = exactRecord(value, ["type", "data"]);
            if (!isText(data.data, 200_000) || !BASE64.test(data.data)) throw "invalid_response";
            return { type, data: data.data };
        }
        case "exit": {
            const data = exactRecord(value, ["type", "code", "reason"]);
            if (!(data.code === null || (typeof data.code === "number" && Number.isInteger(data.code))) || !isText(data.reason, 64)) throw "invalid_response";
            return { type, code: data.code, reason: data.reason };
        }
        case "error": {
            const data = exactRecord(value, ["type", "code", "message"]);
            if (!isText(data.code, 64) || !isText(data.message, 512)) throw "invalid_response";
            return { type, code: data.code, message: data.message };
        }
        case "closed": {
            const data = exactRecord(value, ["type", "reason"]);
            if (!isText(data.reason, 64)) throw "invalid_response";
            return { type, reason: data.reason };
        }
        default:
            throw "invalid_response";
    }
}

/** The raw bytes of an `output` event. */
export function decodeOutput(data: string): Uint8Array {
    const binary = atob(data);
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
    return bytes;
}

const CLOSED_TEXT: Record<string, string> = {
    closed: "session closed",
    session_expired: "session expired",
    slow_consumer: "the client fell behind",
    server_shutdown: "the controller restarted",
    connection_lost: "connection lost",
};

/** Why a session ended, as the footer and the divider say it: `exit 0`, `idle timeout`, … */
export function endedText(exit: { code: number | null; reason: string } | null, closed: string | null): string {
    if (exit) {
        switch (exit.reason) {
            case "exited":
                return exit.code === null ? "exited · exit code unknown" : `exit ${exit.code}`;
            case "idle_timeout":
                return "idle timeout";
            case "max_duration":
                return "8 hour limit";
            case "connection_lost":
                return "connection lost";
            default:
                return exit.reason.split("_").join(" ");
        }
    }
    return CLOSED_TEXT[closed ?? "connection_lost"] ?? "connection lost";
}
