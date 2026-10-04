/**
 * Exec session events crossing the Tauri boundary over a Channel.
 *
 * Mirrors `ExecStreamEvent` in `app/src-tauri/src/session/exec_sessions.rs`; the shared fixture
 * at `app/tests/fixtures/exec-ipc.json` keeps both sides in sync.
 */

export type ExecStreamEvent =
    | { type: "started"; shell: string }
    /** Base64 of the raw bytes the terminal wrote. */
    | { type: "output"; data: string }
    /** `reason` stays open: `exited`, `idle_timeout`, `abandoned`, … and any a newer controller adds. */
    | { type: "exit"; code: number | null; reason: string }
    | { type: "error"; code: string; message: string }
    | { type: "closed"; reason: string };

/** Exec sessions one sign-in may hold at once (ADR 0026). */
export const MAX_EXEC_SESSIONS = 4;
/** The size a session opens at before its terminal has measured itself. */
export const INITIAL_EXEC_SIZE = { cols: 120, rows: 32 } as const;

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
            // The session closed without seeing the shell exit.
            case "abandoned":
                return "the shell may still be running in the container";
            default:
                return exit.reason.split("_").join(" ") || "reason unknown";
        }
    }
    return CLOSED_TEXT[closed ?? "connection_lost"] ?? "connection lost";
}
