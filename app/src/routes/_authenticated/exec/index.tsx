import { useEffect, useState } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { Plus, SquareTerminal } from "lucide-react";
import { PageHeader } from "@/components/page-header";
import { ActionButton } from "@/components/kit/action-button";
import { StatusDot, type Tone } from "@/components/kit/badges";
import { EmptyState } from "@/components/kit/region-states";
import { Tabs, TabsContent } from "@/components/ui/tabs";
import { ExecTabBody } from "@/components/exec/exec-tab-body";
import { ExecTabs } from "@/components/exec/exec-tabs";
import { TargetPicker } from "@/components/exec/target-picker";
import type { ExecTab } from "@/contexts/exec-sessions-context-value";
import { useContainerAction, useContainers } from "@/hooks/use-containers";
import { useExecBuffer, useExecSessions, useOpenShells } from "@/hooks/use-exec-sessions";
import { withIdempotencyKey } from "@/hooks/use-operations";
import { MAX_EXEC_SESSIONS, endedText } from "@/types/exec";
import { cn } from "@/lib/utils";

/** `?node=&container=`: a shell in that container opens on load (a container row's Exec). */
interface ExecSearch {
    node?: string;
    container?: string;
}

const NODE_ID = /^(?!\.{1,2}$)[A-Za-z0-9._-]{1,128}$/;
const CONTAINER_ID = /^[0-9a-f]{64}$/;

export const Route = createFileRoute("/_authenticated/exec/")({
    validateSearch: (search: Record<string, unknown>): ExecSearch => ({
        ...(typeof search.node === "string" && NODE_ID.test(search.node) ? { node: search.node } : {}),
        ...(typeof search.container === "string" && CONTAINER_ID.test(search.container) ? { container: search.container } : {}),
    }),
    component: ExecPage,
});

/** The line under the terminal: connected, disconnected and why, or couldn't start. */
function StatusLine({ tab, open }: { tab: ExecTab; open: number }) {
    const snapshot = useExecBuffer(tab.buffer);
    let tone: Tone = "success";
    let text = `Connected to ${tab.name}`;
    let count = `${open} active ${open === 1 ? "session" : "sessions"}`;
    if (snapshot.status === "connecting") {
        tone = "warning";
        text = `Opening a shell in ${tab.name}…`;
    } else if (snapshot.status === "failed") {
        tone = "destructive";
        text = `Couldn't start a shell in ${tab.name} · ${snapshot.error?.code ?? "error"}`;
        count = `${open} of ${MAX_EXEC_SESSIONS} sessions open`;
    } else if (snapshot.status === "ended") {
        tone = "neutral";
        text = `Disconnected from ${tab.name} · ${snapshot.error ? snapshot.error.code.split("_").join(" ") : endedText(snapshot.exit, snapshot.closed)}`;
    }
    return (
        <div
            role='status'
            aria-label='Session status'
            className='flex h-9 shrink-0 items-center gap-2 rounded-md border border-border bg-card px-3 font-mono text-3xs'
        >
            <StatusDot tone={tone} className='size-1.5' />
            <span className={cn(tone === "destructive" ? "text-destructive" : "text-secondary-foreground")}>{text}</span>
            <span className='ml-auto text-muted-foreground'>{count}</span>
        </div>
    );
}

function ExecPage() {
    const search = Route.useSearch();
    const navigate = Route.useNavigate();
    const exec = useExecSessions();
    const open = useOpenShells();
    const containers = useContainers();
    const [picking, setPicking] = useState(false);
    const [starting, setStarting] = useState<string | null>(null);
    const rows = containers.data?.items;
    const active = exec.tabs.find((tab) => tab.id === exec.activeId) ?? null;
    const full = open >= MAX_EXEC_SESSIONS;

    // START CONTAINER on a can't-start tab: once the start settles, a fresh shell opens there.
    const { tabs, reconnect } = exec;
    const start = useContainerAction((view, variables) => {
        setStarting(null);
        const tab = tabs.find((entry) => entry.nodeId === variables.nodeId && entry.containerId === variables.containerId);
        if (tab && view.state === "succeeded") reconnect(tab.id);
    });

    // Arriving with `?node=&container=` opens that shell once the container list can name it, then
    // drops the parameters.
    const { open: openShell } = exec;
    useEffect(() => {
        if (!search.node || !search.container || !rows) return;
        const row = rows.find((entry) => entry.nodeId === search.node && entry.containerId === search.container);
        openShell({ nodeId: search.node, containerId: search.container, name: row?.name ?? search.container.slice(0, 12) });
        void navigate({ search: {}, replace: true });
    }, [search.node, search.container, rows, openShell, navigate]);

    return (
        <div className='flex h-full min-h-0 flex-col gap-5'>
            <PageHeader title='EXEC' subtitle='// interactive terminal sessions'>
                <ActionButton
                    variant='primary'
                    icon={Plus}
                    restricted={
                        full
                            ? {
                                  title: `Session limit reached · ${open} of ${MAX_EXEC_SESSIONS} open`,
                                  reason: "Close one to start another. Idle shells close on their own after 30 min.",
                              }
                            : undefined
                    }
                    onClick={() => setPicking(true)}
                >
                    NEW SHELL
                </ActionButton>
            </PageHeader>
            {exec.tabs.length === 0 ? (
                <div className='flex flex-1 items-center justify-center rounded-md border border-border bg-card'>
                    <EmptyState
                        icon={SquareTerminal}
                        title='No shells open'
                        primaryAction={{ label: "NEW SHELL", icon: Plus, onClick: () => setPicking(true) }}
                    >
                        Open a shell in a running container, here or from a container's row.
                    </EmptyState>
                </div>
            ) : (
                <>
                    <Tabs
                        value={exec.activeId}
                        onValueChange={(value) => typeof value === "string" && exec.select(value)}
                        className='flex min-h-0 flex-1 flex-col gap-0 overflow-hidden rounded-md border border-border bg-card'
                    >
                        <ExecTabs tabs={exec.tabs} open={open} onClose={exec.close} onNew={() => setPicking(true)} />
                        {exec.tabs.map((tab) => {
                            const service = rows?.find((row) => row.nodeId === tab.nodeId && row.containerId === tab.containerId)?.serviceName ?? null;
                            return (
                                <TabsContent key={tab.id} value={tab.id} className='flex min-h-0 flex-1 flex-col data-[hidden]:hidden'>
                                    <ExecTabBody
                                        tab={tab}
                                        onData={(data) => exec.write(tab.id, data)}
                                        onResize={(cols, rowCount) => exec.resize(tab.id, cols, rowCount)}
                                        onReconnect={(shell) => exec.reconnect(tab.id, shell)}
                                        onClose={() => exec.close(tab.id)}
                                        onStart={() => {
                                            setStarting(tab.id);
                                            start.mutate(
                                                withIdempotencyKey({
                                                    nodeId: tab.nodeId,
                                                    containerId: tab.containerId,
                                                    action: "start",
                                                    options: null,
                                                    name: tab.name,
                                                }),
                                                { onError: () => setStarting(null) },
                                            );
                                        }}
                                        onViewLogs={service ? () => void navigate({ to: "/logs", search: { service } }) : null}
                                        starting={starting === tab.id}
                                    />
                                </TabsContent>
                            );
                        })}
                    </Tabs>
                    {active && <StatusLine tab={active} open={open} />}
                </>
            )}
            <TargetPicker open={picking} onOpenChange={setPicking} onPick={exec.open} />
        </div>
    );
}
