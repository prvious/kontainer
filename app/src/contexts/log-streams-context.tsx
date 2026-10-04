import { useMemo, useRef, useState, type ReactNode } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Channel, invoke } from "@tauri-apps/api/core";
import { LogBuffer } from "@/components/logs/log-buffer";
import { LogStreamsContext, type LogStreamsContextValue, type LogTab } from "@/contexts/log-streams-context-value";
import { authenticatedInvoke } from "@/hooks/authenticated-invoke";
import { useAppSettings } from "@/hooks/use-app-settings";
import { useAuth } from "@/hooks/use-auth";
import { listServices, servicesKey } from "@/hooks/use-services";
import { DEFAULT_LOG_SCROLLBACK, DEFAULT_LOG_TAIL, MAX_LOG_TAIL, type LogStreamEvent } from "@/types/logs";

const HOUR_MS = 60 * 60 * 1000;

interface OpenVariables {
    tab: LogTab;
    since: number | null;
    tail: number;
    /** Look the service up by name first: a redeploy gives the same name a new id. */
    lookUp: boolean;
}

/**
 * The Logs tabs for this window (mounted in the authenticated layout, so tabs and their streams
 * survive navigating away from `/logs`). Each stream's Channel feeds its tab's buffer; Rust
 * cancels every stream on logout, window close, and reload.
 */
export function LogStreamsProvider({ children }: { children: ReactNode }) {
    const { session, reportInventoryCustodyError } = useAuth();
    const settings = useAppSettings();
    const queryClient = useQueryClient();
    const profileId = session?.profileId ?? "";
    const [tabs, setTabs] = useState<LogTab[]>([]);
    const [activeId, setActiveId] = useState<string | null>(null);
    // Tabs by id for callbacks that outlive the render that started them.
    const live = useRef(new Map<string, LogTab>());
    // Each tab's current Channel, so a replaced stream's late events can't reach the new buffer.
    const channels = useRef(new Map<string, Channel<LogStreamEvent>>());

    const { mutate: openStream } = useMutation({
        mutationFn: async ({ tab, since, tail, lookUp }: OpenVariables) => {
            let serviceId = tab.serviceId;
            if (lookUp) {
                // A fresh list, not the cached one: right after a redeploy the cache still has the old id.
                const services = await queryClient
                    .fetchQuery({ queryKey: servicesKey(profileId), queryFn: () => listServices(profileId, reportInventoryCustodyError), staleTime: 0 })
                    // Without the list, reopen the id we had and let the stream say what's wrong.
                    .catch(() => null);
                serviceId = services?.items.find((service) => service.name === tab.serviceName)?.id ?? serviceId;
                // The tab closed or started over while the list loaded.
                if (live.current.get(tab.id) !== tab) return null;
            }
            const channel = new Channel<LogStreamEvent>();
            channel.onmessage = (event) => tab.buffer.apply(event, Date.now());
            channels.current.set(tab.id, channel);
            const streamId = await authenticatedInvoke<string>("logs_open", profileId, { serviceId, since, tail, channel }, reportInventoryCustodyError);
            return { streamId, serviceId };
        },
        onSuccess: (result, { tab }) => {
            if (!result) return;
            const { streamId, serviceId } = result;
            if (!live.current.has(tab.id)) {
                // The tab closed while its stream was opening.
                closeStream(streamId);
                return;
            }
            const opened = { ...(live.current.get(tab.id) ?? tab), serviceId, streamId };
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
        const detach = (id: string) => {
            const channel = channels.current.get(id);
            if (channel) channel.onmessage = () => {};
            channels.current.delete(id);
        };
        const start = (tab: LogTab, since: number | null, tail: number) => {
            const next = { ...tab, streamId: null, window: { since, tail, openedAt: Date.now() } };
            live.current.set(tab.id, next);
            setTabs((current) => current.map((entry) => (entry.id === tab.id ? next : entry)));
            openStream({ tab: next, since, tail, lookUp: true });
        };
        const replace = (tab: LogTab) => {
            detach(tab.id);
            if (tab.streamId) closeStream(tab.streamId);
        };
        return {
            tabs,
            activeId,
            select: setActiveId,
            open(service) {
                const existing = [...live.current.values()].find((tab) => tab.serviceName === service.name);
                if (existing) {
                    setActiveId(existing.id);
                    return;
                }
                const scrollback = settings.data?.terminalScrollback ?? DEFAULT_LOG_SCROLLBACK;
                const tab: LogTab = {
                    id: crypto.randomUUID(),
                    serviceId: service.id,
                    serviceName: service.name,
                    streamId: null,
                    buffer: new LogBuffer(scrollback),
                    window: { since: null, tail: DEFAULT_LOG_TAIL, openedAt: Date.now() },
                };
                live.current.set(tab.id, tab);
                setTabs((current) => [...current, tab]);
                setActiveId(tab.id);
                openStream({ tab, since: null, tail: DEFAULT_LOG_TAIL, lookUp: false });
            },
            close(id) {
                const tab = live.current.get(id);
                live.current.delete(id);
                detach(id);
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
