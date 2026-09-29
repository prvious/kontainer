import { useCallback, useContext, useSyncExternalStore } from "react";
import type { ExecBuffer } from "@/components/exec/exec-buffer";
import { ExecSessionsContext } from "@/contexts/exec-sessions-context-value";

/** The window's Exec tabs. */
export function useExecSessions() {
    const value = useContext(ExecSessionsContext);
    if (!value) throw new Error("useExecSessions outside ExecSessionsProvider");
    return value;
}

/** A session's state, re-rendering as it changes; output goes to the terminal directly. */
export function useExecBuffer(buffer: ExecBuffer) {
    return useSyncExternalStore(buffer.subscribe, buffer.getSnapshot);
}

const NO_TABS: never[] = [];

/** Shells starting or running, for the `+` limit, the status line, and the status bar. */
export function useOpenShells(): number {
    const tabs = useContext(ExecSessionsContext)?.tabs ?? NO_TABS;
    const subscribe = useCallback(
        (listener: () => void) => {
            const unsubscribers = tabs.map((tab) => tab.buffer.subscribe(listener));
            return () => {
                for (const unsubscribe of unsubscribers) unsubscribe();
            };
        },
        [tabs],
    );
    return useSyncExternalStore(subscribe, () =>
        tabs.reduce((open, tab) => {
            const status = tab.buffer.getSnapshot().status;
            return status === "active" || status === "connecting" ? open + 1 : open;
        }, 0),
    );
}
