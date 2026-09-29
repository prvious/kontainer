import { useState } from "react";
import { Link, createFileRoute } from "@tanstack/react-router";
import { Box, Loader2, RefreshCw, Upload } from "lucide-react";
import { PageHeader } from "@/components/page-header";
import { ActionButton } from "@/components/kit/action-button";
import { InlineAlert } from "@/components/kit/inline-alert";
import { EmptyState, ErrorState } from "@/components/kit/region-states";
import { SkeletonStat } from "@/components/kit/skeletons";
import { ContainersTable } from "@/components/containers/containers-table";
import { RemoveContainerDialog } from "@/components/containers/remove-container-dialog";
import { useRunContainerAction } from "@/components/containers/use-run-container-action";
import { applyFilters, parseContainerSearch, type StateFilter } from "@/components/containers/container-search";
import { OverviewCards } from "@/components/overview/overview-cards";
import { containerCounts, cpuPeak } from "@/components/overview/overview-metrics";
import { OverviewTableControls } from "@/components/overview/overview-table-controls";
import { TraefikBanner } from "@/components/overview/traefik-banner";
import { useAuth } from "@/hooks/use-auth";
import { useConnectivity } from "@/hooks/use-connectivity";
import { useContainers, type ContainerRow } from "@/hooks/use-containers";
import { useClusterSummary, useNodes, useSamples } from "@/hooks/use-nodes";
import { useNow } from "@/hooks/use-now";
import { isSettled, useTrackedOperations } from "@/hooks/use-operation";
import { clockTime, relativeTime, shortDuration } from "@/lib/time";
import type { ClusterSummary } from "@/types/nodes";

/** `?q=&state=`: the running-containers table's search and state filter. */
interface OverviewSearch {
    q?: string;
    state?: StateFilter[];
}

export const Route = createFileRoute("/_authenticated/")({
    validateSearch: (raw: Record<string, unknown>): OverviewSearch => {
        const { q, state } = parseContainerSearch(raw);
        return { ...(q ? { q } : {}), ...(state ? { state } : {}) };
    },
    component: OverviewPage,
});

/** The table shows the 50 busiest; VIEW ALL opens the Containers page. */
const OVERVIEW_ROW_CAP = 50;
/** How long the reconnected alert stays after the controller answers again. */
const RECONNECTED_ALERT_MS = 5_000;
const NOT_BUILT = { title: "Not built yet", reason: "Deploying stacks from Kontainer isn't part of this build; use docker stack deploy." };
const plural = (count: number, one: string) => `${count} ${one}${count === 1 ? "" : "s"}`;

function swarmState(summary: ClusterSummary): string {
    if (summary.managersTotal === 0) return "no swarm manager";
    return summary.quorum === "healthy" ? "swarm active" : "swarm degraded";
}

/** CPU first, nulls last; the free-text search matches name, id, and image like the Containers page. */
function busiestFirst(rows: ContainerRow[], query: string): ContainerRow[] {
    const needle = query.trim().toLowerCase();
    const cpu = (row: ContainerRow) => (row.state === "running" && !row.stale ? row.cpuPercent : null);
    return rows
        .filter((row) => needle === "" || `${row.name} ${row.containerId} ${row.image}`.toLowerCase().includes(needle))
        .sort((left, right) => (cpu(right) ?? -1) - (cpu(left) ?? -1));
}

function OverviewPage() {
    const search = Route.useSearch();
    const navigate = Route.useNavigate();
    const { session } = useAuth();
    const summary = useClusterSummary();
    const nodes = useNodes();
    const hour = useSamples("cluster", "1h", "60s");
    const containers = useContainers();
    const connectivity = useConnectivity();
    const tracked = useTrackedOperations("containers");
    const run = useRunContainerAction();
    const now = useNow();
    const [removing, setRemoving] = useState<ContainerRow | null>(null);

    const queries = [summary, nodes, hour, containers];
    const firstLoad = summary.data === undefined || containers.data === undefined;
    const syncedAt = summary.dataUpdatedAt > 0 ? summary.dataUpdatedAt : null;
    const staleAt = connectivity.reachable ? null : syncedAt;
    const reconnected = connectivity.reachable && connectivity.reconnectedAt !== null && now - connectivity.reconnectedAt < RECONNECTED_ALERT_MS;
    const label = session?.label ?? (session ? new URL(session.origin).host : "");

    const rows = containers.data?.items ?? [];
    const matching = busiestFirst(applyFilters(rows, { state: search.state }), search.q ?? "");
    const shown = matching.slice(0, OVERVIEW_ROW_CAP);
    const pending = new Set<string>();
    for (const { operation, view } of tracked) if (!view || !isSettled(view.state)) for (const target of operation.targets ?? []) pending.add(target);

    function setSearch(next: OverviewSearch) {
        void navigate({ search: next, replace: true });
    }

    let subtitle;
    if (firstLoad) {
        subtitle = (
            <span className='flex items-center gap-1.5'>
                <Loader2 aria-hidden className='size-3 animate-spin text-primary' />
                syncing cluster state…
            </span>
        );
    } else {
        const facts = [`for ${label}`, plural(summary.data.nodesTotal, "node"), swarmState(summary.data)];
        subtitle =
            staleAt !== null ? (
                <span className='text-warning'>{[...facts, `stale — last synced ${clockTime(staleAt, true)}`].join(" · ")}</span>
            ) : (
                [...facts, syncedAt ? `synced ${shortDuration((now - syncedAt) / 1000)} ago` : "not synced yet"].join(" · ")
            );
    }

    const filtering = Boolean(search.q) || (search.state?.length ?? 0) > 0;
    const emptyState = filtering ? (
        <EmptyState title='No containers match' primaryAction={{ label: "CLEAR", onClick: () => setSearch({}) }}>
            Nothing matches this search and state filter.
        </EmptyState>
    ) : (
        <EmptyState icon={Box} title='No containers yet'>
            Containers show up here as soon as a stack or service schedules a task on one of your nodes.
        </EmptyState>
    );
    const errorState =
        containers.error && !containers.data ? (
            <ErrorState
                title="Couldn't load containers"
                details={{ code: String(containers.error), message: "The container inventory request failed. Nothing was changed." }}
                onRetry={() => void containers.refetch()}
            />
        ) : undefined;

    return (
        <div className='flex flex-col gap-5'>
            <PageHeader title='CLUSTER OVERVIEW' subtitle={subtitle}>
                <ActionButton
                    variant='outline'
                    icon={RefreshCw}
                    pending={queries.some((query) => query.isFetching)}
                    onClick={() => {
                        for (const query of queries) void query.refetch();
                    }}
                >
                    REFRESH
                </ActionButton>
                <ActionButton variant='primary' icon={Upload} restricted={NOT_BUILT}>
                    DEPLOY
                </ActionButton>
            </PageHeader>
            {!connectivity.reachable && (
                <InlineAlert
                    tone='warning'
                    title='Showing cached data — controller unreachable'
                    action={{ label: "RETRY NOW", icon: RefreshCw, pending: connectivity.retrying, onClick: connectivity.retry }}
                >
                    {syncedAt
                        ? `Last successful sync ${clockTime(syncedAt, true)} (${relativeTime(syncedAt, now)}). Changes are paused until the connection returns.`
                        : "No successful sync yet. Changes are paused until the connection returns."}
                </InlineAlert>
            )}
            {reconnected && (
                <InlineAlert tone='success' title='Reconnected — the controller is reachable again'>
                    Figures are live again and changes are enabled.
                </InlineAlert>
            )}
            {firstLoad ? (
                <div aria-label='Loading cluster figures' aria-busy className='grid grid-cols-2 gap-4 xl:grid-cols-4'>
                    {[0, 1, 2, 3].map((index) => (
                        <SkeletonStat key={index} className='min-h-30' />
                    ))}
                </div>
            ) : (
                <OverviewCards
                    counts={containerCounts(rows)}
                    summary={summary.data}
                    nodes={nodes.data?.items ?? []}
                    peak={cpuPeak(hour.data?.points ?? [])}
                    staleAt={staleAt}
                />
            )}
            <TraefikBanner />
            <ContainersTable
                title={staleAt !== null ? `RUNNING CONTAINERS · AS OF ${clockTime(staleAt)}` : "RUNNING CONTAINERS"}
                rows={shown}
                snapshotAt={containers.data?.snapshotAt ?? new Date(0).toISOString()}
                search=''
                // Rows arrive busiest first and capped; the table keeps that order.
                sorting={[]}
                onSortingChange={() => {}}
                headerRight={
                    <>
                        <OverviewTableControls
                            search={search.q ?? ""}
                            onSearchChange={(q) => setSearch({ ...search, q: q || undefined })}
                            states={search.state ?? []}
                            onStatesChange={(state) => setSearch({ ...search, state: state.length > 0 ? state : undefined })}
                        />
                        <Link
                            to='/containers'
                            search={{ q: search.q, state: search.state }}
                            className='font-mono text-3xs font-bold tracking-[0.5px] text-sidebar-accent-foreground hover:underline'
                        >
                            VIEW ALL
                        </Link>
                    </>
                }
                failures={new Map()}
                pending={pending}
                onAction={(row, action) => run(row, action)}
                onRemove={setRemoving}
                onLogs={(row) => void navigate({ to: "/logs", search: { service: row.serviceName ?? undefined } })}
                countLabel={matching.length > shown.length ? `${shown.length} OF ${matching.length}` : String(matching.length)}
                isFirstLoad={containers.isPending}
                isRefetching={containers.isFetching && !containers.isPending}
                dimmed={staleAt !== null}
                emptyState={emptyState}
                errorState={errorState}
            />
            <RemoveContainerDialog row={removing} onClose={() => setRemoving(null)} />
        </div>
    );
}
