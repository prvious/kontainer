import { useCallback, useMemo, useRef, useState, type ReactNode } from "react";
import { useMutation } from "@tanstack/react-query";
import { Channel, invoke } from "@tauri-apps/api/core";
import { ExecBuffer } from "@/components/exec/exec-buffer";
import { ExecSessionsContext, type ExecSessionsContextValue, type ExecTab } from "@/contexts/exec-sessions-context-value";
import { authenticatedInvoke } from "@/hooks/authenticated-invoke";
import { useAuth } from "@/hooks/use-auth";
import { useSettings } from "@/hooks/use-settings";
import { INITIAL_EXEC_SIZE, type ExecStreamEvent } from "@/types/exec";

/**
 * The Exec tabs for this window (mounted in the authenticated layout, so shells survive leaving
 * `/exec`). Each session's Channel feeds its tab's buffer; Rust ends every session on logout and
 * window close.
 */
export function ExecSessionsProvider({ children }: { children: ReactNode }) {
    const { session, reportInventoryCustodyError } = useAuth();
    const { terminalDefaults } = useSettings();
    const defaultShell = terminalDefaults.shell.trim() || null;
    const profileId = session?.profileId ?? "";
    const [tabs, setTabs] = useState<ExecTab[]>([]);
    const [activeId, setActiveId] = useState<string | null>(null);
    // Tabs by id for callbacks that outlive the render that started them.
    const live = useRef(new Map<string, ExecTab>());

    const { mutate: resizeSession } = useMutation({
        mutationFn: ({ sessionId, cols, rows }: { sessionId: string; cols: number; rows: number }) => invoke("exec_resize", { sessionId, cols, rows }),
    });
    // The size each tab's terminal last measured. A terminal fits before its shell starts, so the
    // size is sent once the session is live as well as on every change after.
    const sizes = useRef(new Map<string, { cols: number; rows: number }>());
    const sendSize = useCallback(
        (id: string) => {
            const tab = live.current.get(id);
            const size = sizes.current.get(id);
            if (tab?.sessionId && size && tab.buffer.getSnapshot().status === "active") resizeSession({ sessionId: tab.sessionId, ...size });
        },
        [resizeSession],
    );

    const { mutate: closeSession } = useMutation({ mutationFn: (sessionId: string) => invoke("exec_close", { sessionId }) });
    const { mutate: openSession } = useMutation({
        mutationFn: (tab: ExecTab) => {
            const channel = new Channel<ExecStreamEvent>();
            channel.onmessage = (event) => {
                tab.buffer.apply(event, Date.now());
                if (event.type === "started") sendSize(tab.id);
            };
            return authenticatedInvoke<string>(
                "exec_open",
                profileId,
                { nodeId: tab.nodeId, containerId: tab.containerId, shell: tab.shell, ...INITIAL_EXEC_SIZE, channel },
                reportInventoryCustodyError,
            );
        },
        onSuccess: (sessionId, tab) => {
            if (live.current.get(tab.id)?.buffer !== tab.buffer) {
                // The tab closed or reconnected while this session was opening.
                closeSession(sessionId);
                return;
            }
            const opened = { ...tab, sessionId };
            live.current.set(tab.id, opened);
            setTabs((current) => current.map((entry) => (entry.id === tab.id ? opened : entry)));
            sendSize(tab.id);
        },
        onError: (code, tab) => {
            tab.buffer.apply({ type: "error", code: String(code), message: "The shell could not be opened." }, Date.now());
        },
    });
    const { mutate: writeSession } = useMutation({
        mutationFn: ({ sessionId, data }: { sessionId: string; data: string }) => invoke("exec_write", { sessionId, data }),
    });

    // The actions read tabs through `live`, so they keep their identity as tabs change.
    const actions = useMemo<Omit<ExecSessionsContextValue, "tabs" | "activeId">>(() => {
        const put = (tab: ExecTab) => {
            live.current.set(tab.id, tab);
            setTabs((current) =>
                current.some((entry) => entry.id === tab.id) ? current.map((entry) => (entry.id === tab.id ? tab : entry)) : [...current, tab],
            );
        };
        const activeSession = (id: string) => {
            const tab = live.current.get(id);
            return tab?.sessionId && tab.buffer.getSnapshot().status === "active" ? tab.sessionId : null;
        };
        return {
            select: setActiveId,
            open(target) {
                const tab: ExecTab = {
                    ...target,
                    id: crypto.randomUUID(),
                    sessionId: null,
                    buffer: new ExecBuffer(),
                    shell: defaultShell,
                };
                put(tab);
                setActiveId(tab.id);
                openSession(tab);
            },
            close(id) {
                const tab = live.current.get(id);
                live.current.delete(id);
                sizes.current.delete(id);
                if (tab?.sessionId && tab.buffer.getSnapshot().status === "active") closeSession(tab.sessionId);
                const remaining = [...live.current.keys()];
                setTabs((current) => current.filter((entry) => entry.id !== id));
                setActiveId((active) => (active === id ? (remaining[remaining.length - 1] ?? null) : active));
            },
            reconnect(id, shell) {
                const tab = live.current.get(id);
                if (!tab) return;
                if (tab.sessionId && tab.buffer.getSnapshot().status === "active") closeSession(tab.sessionId);
                const next: ExecTab = { ...tab, sessionId: null, buffer: new ExecBuffer(), shell: shell === undefined ? tab.shell : shell };
                put(next);
                openSession(next);
            },
            write(id, data) {
                const sessionId = activeSession(id);
                if (sessionId) writeSession({ sessionId, data });
            },
            resize(id, cols, rows) {
                if (cols <= 0 || rows <= 0) return;
                sizes.current.set(id, { cols, rows });
                sendSize(id);
            },
        };
    }, [openSession, closeSession, writeSession, sendSize, defaultShell]);
    const value = useMemo<ExecSessionsContextValue>(() => ({ tabs, activeId, ...actions }), [tabs, activeId, actions]);

    return <ExecSessionsContext value={value}>{children}</ExecSessionsContext>;
}
