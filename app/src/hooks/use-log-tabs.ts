import { useContext, useSyncExternalStore } from "react";
import type { LogBuffer } from "@/components/logs/log-buffer";
import { LogStreamsContext } from "@/contexts/log-streams-context-value";

/** The window's Logs tabs; outside the authenticated layout there are none. */
export function useLogStreams() {
    const value = useContext(LogStreamsContext);
    if (!value) throw new Error("useLogStreams outside LogStreamsProvider");
    return value;
}

/** How many Logs tabs are open, for the status bar; 0 outside the provider. */
export function useOpenLogTabs(): number {
    return useContext(LogStreamsContext)?.tabs.length ?? 0;
}

/** A tab's buffer snapshot, re-rendering as lines arrive. */
export function useLogBuffer(buffer: LogBuffer) {
    return useSyncExternalStore(buffer.subscribe, buffer.getSnapshot);
}
