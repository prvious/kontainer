import { useEffect, useState } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { Plus, ScrollText, Search, WrapText } from "lucide-react";
import { PageHeader } from "@/components/page-header";
import { ActionButton } from "@/components/kit/action-button";
import { EmptyState } from "@/components/kit/region-states";
import { LabelTooltip } from "@/components/kit/tooltips";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent } from "@/components/ui/tabs";
import { LogTabBody } from "@/components/logs/log-tab-body";
import { LogTabs } from "@/components/logs/log-tabs";
import { StreamPicker } from "@/components/logs/stream-picker";
import { useLogBuffer, useLogStreams } from "@/hooks/use-log-tabs";
import { useServices } from "@/hooks/use-services";
import type { LogTab } from "@/contexts/log-streams-context-value";
import { lineMatcher } from "@/types/logs";
import { cn } from "@/lib/utils";

/** `?service=`: a service id or name whose tab opens on load. */
interface LogsSearch {
    service?: string;
}

const SERVICE_REF = /^[A-Za-z0-9][A-Za-z0-9_.-]{0,62}$/;

export const Route = createFileRoute("/_authenticated/logs/")({
    validateSearch: (search: Record<string, unknown>): LogsSearch =>
        typeof search.service === "string" && SERVICE_REF.test(search.service) ? { service: search.service } : {},
    component: LogsPage,
});

function MatchCount({ tab, filter }: { tab: LogTab; filter: string }) {
    const snapshot = useLogBuffer(tab.buffer);
    const matches = lineMatcher(filter);
    const count = snapshot.rows.filter((row) => row.kind === "line" && matches(row.line.text)).length;
    return (
        <span className='font-mono text-3xs text-muted-foreground'>
            {count.toLocaleString()} {count === 1 ? "match" : "matches"}
        </span>
    );
}

function LogsPage() {
    const search = Route.useSearch();
    const navigate = Route.useNavigate();
    const logs = useLogStreams();
    const services = useServices();
    const [picking, setPicking] = useState(false);
    const [filter, setFilter] = useState("");
    const [wrap, setWrap] = useState(false);

    const serviceList = services.data?.items;
    const nameOf = (serviceId: string) => serviceList?.find((service) => service.id === serviceId)?.name ?? serviceId;
    const active = logs.tabs.find((tab) => tab.id === logs.activeId) ?? null;

    // Arriving with `?service=` (from a Services or Containers row) opens that service's stream once
    // the service list can resolve a name to an id, then drops the parameter.
    const { open } = logs;
    useEffect(() => {
        if (!search.service || !serviceList) return;
        const match = serviceList.find((service) => service.id === search.service || service.name === search.service);
        if (match) open(match.id);
        void navigate({ search: {}, replace: true });
    }, [search.service, serviceList, open, navigate]);

    return (
        <div className='flex h-full min-h-0 flex-col gap-5'>
            <PageHeader title='LOGS' subtitle='// per-service log streams'>
                {active && filter.trim() !== "" && <MatchCount tab={active} filter={filter} />}
                <label className='flex h-8 w-60 items-center gap-2 rounded-md border border-border px-3 focus-within:border-ring'>
                    <Search aria-hidden className='size-3.5 shrink-0 text-muted-foreground' />
                    <Input
                        type='search'
                        aria-label='Filter logs'
                        placeholder='Filter logs… (/regex/)'
                        value={filter}
                        onChange={(event) => setFilter(event.target.value)}
                        className='h-auto border-0 bg-transparent p-0 font-mono text-2xs shadow-none focus-visible:ring-0 dark:bg-transparent'
                    />
                </label>
                <LabelTooltip label={wrap ? "Don't wrap lines" : "Wrap lines"}>
                    <button
                        type='button'
                        aria-label='Wrap lines'
                        aria-pressed={wrap}
                        onClick={() => setWrap((current) => !current)}
                        className={cn(
                            "flex size-8 items-center justify-center rounded-md border border-border text-muted-foreground hover:text-foreground",
                            wrap && "border-primary text-sidebar-accent-foreground",
                        )}
                    >
                        <WrapText aria-hidden className='size-3.5' />
                    </button>
                </LabelTooltip>
                <ActionButton variant='primary' icon={Plus} onClick={() => setPicking(true)}>
                    NEW STREAM
                </ActionButton>
            </PageHeader>
            {logs.tabs.length === 0 ? (
                <div className='flex flex-1 items-center justify-center rounded-md border border-border bg-card'>
                    <EmptyState
                        icon={ScrollText}
                        title='No log streams open'
                        primaryAction={{ label: "NEW STREAM", icon: Plus, onClick: () => setPicking(true) }}
                    >
                        Follow a service's logs from every task, live. Open one here, or from a service's row menu.
                    </EmptyState>
                </div>
            ) : (
                <Tabs
                    value={logs.activeId}
                    onValueChange={(value) => typeof value === "string" && logs.select(value)}
                    className='flex min-h-0 flex-1 flex-col gap-0 overflow-hidden rounded-md border border-border bg-card'
                >
                    <LogTabs tabs={logs.tabs} nameOf={nameOf} onClose={logs.close} onNew={() => setPicking(true)} />
                    {logs.tabs.map((tab) => (
                        <TabsContent key={tab.id} value={tab.id} className='flex min-h-0 flex-1 flex-col data-[hidden]:hidden'>
                            <LogTabBody
                                tab={tab}
                                name={nameOf(tab.serviceId)}
                                filter={filter}
                                wrap={wrap}
                                onClearFilter={() => setFilter("")}
                                onLoadPreviousHour={() => logs.loadPreviousHour(tab.id)}
                                onResume={() => logs.resume(tab.id)}
                                onRetryNow={() => logs.retryNow(tab.id)}
                            />
                        </TabsContent>
                    ))}
                </Tabs>
            )}
            <StreamPicker open={picking} onOpenChange={setPicking} onPick={logs.open} />
        </div>
    );
}
