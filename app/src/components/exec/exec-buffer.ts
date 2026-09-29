import { decodeOutput, type ExecStreamEvent } from "@/types/exec";

export type ExecStatus = "connecting" | "active" | "ended" | "failed";

export interface ExecSnapshot {
    status: ExecStatus;
    shell: string | null;
    exit: { code: number | null; reason: string } | null;
    /** Why the shell could not start, or the error that ended it. */
    error: { code: string; message: string } | null;
    closed: string | null;
    endedAt: number | null;
}

const CONNECTING: ExecSnapshot = { status: "connecting", shell: null, exit: null, error: null, closed: null, endedAt: null };
/** Output kept for a terminal that mounts again (leaving and returning to `/exec`). */
const MAX_REPLAY_BYTES = 2 * 1024 * 1024;

/**
 * One session's state and output, fed by its Channel. State is an external store read with
 * `useSyncExternalStore`; output bytes go straight to the mounted terminal and are kept (up to
 * 2 MiB) so a terminal that mounts later can replay them.
 */
export class ExecBuffer {
    #snapshot: ExecSnapshot = CONNECTING;
    #listeners = new Set<() => void>();
    #output = new Set<(bytes: Uint8Array) => void>();
    #chunks: Uint8Array[] = [];
    #bytes = 0;

    subscribe = (listener: () => void) => {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    };

    getSnapshot = () => this.#snapshot;

    /** Live output for a mounted terminal; returns the unsubscribe. */
    onOutput(listener: (bytes: Uint8Array) => void) {
        this.#output.add(listener);
        return () => {
            this.#output.delete(listener);
        };
    }

    /** Everything kept so far, oldest first. */
    replay(): readonly Uint8Array[] {
        return this.#chunks;
    }

    #set(next: Partial<ExecSnapshot>) {
        this.#snapshot = { ...this.#snapshot, ...next };
        for (const listener of this.#listeners) listener();
    }

    apply(event: ExecStreamEvent, now: number) {
        const current = this.#snapshot;
        if (current.status === "ended" || current.status === "failed") return;
        switch (event.type) {
            case "started":
                this.#set({ status: "active", shell: event.shell });
                return;
            case "output": {
                const bytes = decodeOutput(event.data);
                this.#chunks.push(bytes);
                this.#bytes += bytes.length;
                while (this.#bytes > MAX_REPLAY_BYTES && this.#chunks.length > 1) this.#bytes -= this.#chunks.shift()?.length ?? 0;
                for (const listener of this.#output) listener(bytes);
                return;
            }
            case "exit":
                this.#set({ status: "ended", exit: { code: event.code, reason: event.reason }, endedAt: now });
                return;
            case "error":
                this.#set({ status: current.status === "connecting" ? "failed" : "ended", error: { code: event.code, message: event.message }, endedAt: now });
                return;
            case "closed":
                this.#set(
                    current.status === "connecting"
                        ? {
                              status: "failed",
                              closed: event.reason,
                              error: { code: event.reason, message: "The session closed before the shell started." },
                              endedAt: now,
                          }
                        : { status: "ended", closed: event.reason, endedAt: now },
                );
                return;
        }
    }
}
