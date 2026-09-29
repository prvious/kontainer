import { useMemo, useRef, useState, type ReactNode } from "react";
import { useMutation } from "@tanstack/react-query";
import { Channel, invoke } from "@tauri-apps/api/core";
import { LogBuffer } from "@/components/logs/log-buffer";
import { LogStreamsContext, type LogStreamsContextValue, type LogTab } from "@/contexts/log-streams-context-value";
import { authenticatedInvoke } from "@/hooks/authenticated-invoke";
import { useAppSettings } from "@/hooks/use-app-settings";
import { useAuth } from "@/hooks/use-auth";
import { DEFAULT_LOG_SCROLLBACK, DEFAULT_LOG_TAIL, MAX_LOG_TAIL, logStreamEventFromIpc } from "@/types/logs";

const HOUR_MS = 60 * 60 * 1000;
const STREAM_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

function streamIdFromIpc(value: unknown): string {
    if (typeof value !== "string" || !STREAM_ID.test(value)) throw "invalid_response";
    return value;
}

interface OpenVariables {
    tab: LogTab;
    since: number | null;
    tail: number;
}

/**
 * The Logs tabs for this window (mounted in the authenticated layout, so tabs and their streams
 * survive navigating away from `/logs`). Each stream's Channel feeds its tab's buffer; Rust
 * cancels every stream on logout and window close.
 */
export function LogStreamsProvider({ children }: { children: ReactNode }) {
    const { session, reportInventoryCustodyError } = useAuth();
    const settings = useAppSettings();
    const profileId = session?.profileId ?? "";
    const [tabs, setTabs] = useState<LogTab[]>([]);
    const [activeId, setActiveId] = useState<string | null>(null);
    // Tabs by id for callbacks that outlive the render that started them.
    const live = useRef(new Map<string, LogTab>());

    const { mutate: openStream } = useMutation({
        mutationFn: ({ tab, since, tail }: OpenVariables) => {
            const channel = new Channel<unknown>();
            channel.onmessage = (raw) => {
                try {
                    tab.buffer.apply(logStreamEventFromIpc(raw), Date.now());
                } catch {
                    tab.buffer.apply(
                        { type: "ended", reason: "error", code: "invalid_response", message: "The log stream sent something unexpected." },
                        Date.now(),
                    );
                }
            };
            return authenticatedInvoke(
                "logs_open",
                profileId,
                { serviceId: tab.serviceId, since, tail, channel },
                streamIdFromIpc,
                reportInventoryCustodyError,
            );
        },
        onSuccess: (streamId, { tab }) => {
            if (!live.current.has(tab.id)) {
                // The tab closed while its stream was opening.
                closeStream(streamId);
                return;
            }
            const opened = { ...(live.current.get(tab.id) ?? tab), streamId };
            live.current.set(tab.id, opened);
            tab.buffer.opened();
            setTabs((current) => current.map((entry) => (entry.id === tab.id ? opened : entry)));
        },
        onError: (code, { tab }) => {
            tab.buffer.apply({ type: "ended", reason: "error", code: String(code), message: "The log stream could not be opened." }, Date.now());
        },
    });
    const { mutate: closeStream } = useMutation({ mutationFn: (streamId: string) => invoke("logs_close", { streamId }) });
    const { mutate: retryStream } = useMutation({ mutationFn: (streamId: string) => invoke("logs_retry_now", { streamId }) });

    const value = useMemo<LogStreamsContextValue>(() => {
        const start = (tab: LogTab, since: number | null, tail: number) => {
            const next = { ...tab, streamId: null, window: { since, tail, openedAt: Date.now() } };
            live.current.set(tab.id, next);
            setTabs((current) => current.map((entry) => (entry.id === tab.id ? next : entry)));
            openStream({ tab: next, since, tail });
        };
        const replace = (tab: LogTab) => {
            if (tab.streamId) closeStream(tab.streamId);
        };
        return {
            tabs,
            activeId,
            select: setActiveId,
            open(serviceId) {
                const existing = [...live.current.values()].find((tab) => tab.serviceId === serviceId);
                if (existing) {
                    setActiveId(existing.id);
                    return;
                }
                const scrollback = settings.data?.terminalScrollback ?? DEFAULT_LOG_SCROLLBACK;
                const tab: LogTab = {
                    id: crypto.randomUUID(),
                    serviceId,
                    streamId: null,
                    buffer: new LogBuffer(scrollback),
                    window: { since: null, tail: DEFAULT_LOG_TAIL, openedAt: Date.now() },
                };
                live.current.set(tab.id, tab);
                setTabs((current) => [...current, tab]);
                setActiveId(tab.id);
                openStream({ tab, since: null, tail: DEFAULT_LOG_TAIL });
            },
            close(id) {
                const tab = live.current.get(id);
                live.current.delete(id);
                if (tab?.streamId) closeStream(tab.streamId);
                const remaining = [...live.current.keys()];
                setTabs((current) => current.filter((entry) => entry.id !== id));
                setActiveId((active) => (active === id ? (remaining[remaining.length - 1] ?? null) : active));
            },
            resume(id) {
                const tab = live.current.get(id);
                if (!tab) return;
                replace(tab);
                const since = tab.buffer.getSnapshot().lastTs;
                tab.buffer.restart(true);
                start(tab, since, since === null ? DEFAULT_LOG_TAIL : MAX_LOG_TAIL);
            },
            loadPreviousHour(id) {
                const tab = live.current.get(id);
                if (!tab) return;
                replace(tab);
                tab.buffer.restart(false);
                start(tab, Date.now() - HOUR_MS, MAX_LOG_TAIL);
            },
            retryNow(id) {
                const streamId = live.current.get(id)?.streamId;
                if (streamId) retryStream(streamId);
            },
        };
    }, [tabs, activeId, openStream, closeStream, retryStream, settings.data?.terminalScrollback]);

    return <LogStreamsContext value={value}>{children}</LogStreamsContext>;
}
