import { useState } from "react";
import { createFileRoute } from "@tanstack/react-router";
import type { RowSelectionState } from "@tanstack/react-table";
import { Box, Pause, Play, RefreshCw, RotateCw, Square, Trash2 } from "lucide-react";
import { PageHeader } from "@/components/page-header";
import { ActionButton } from "@/components/kit/action-button";
import { ConfirmDialog } from "@/components/kit/confirm-dialog";
import { EmptyState, ErrorState } from "@/components/kit/region-states";
import { InlineAlert } from "@/components/kit/inline-alert";
import { notify } from "@/components/kit/notify";
import { TableToolbar, type FilterChip } from "@/components/data-table/table-toolbar";
import { BulkBar } from "@/components/data-table/bulk-bar";
import { PinnedOperations } from "@/components/operations/pinned-operations";
import { ContainerStatesStrip } from "@/components/containers/container-states-strip";
import { ContainersTable } from "@/components/containers/containers-table";
import { ContainerFilters } from "@/components/containers/container-filters";
import {
    CONTAINER_SORTS,
    SORT_LABELS,
    activeFilterCount,
    applyFilters,
    parseContainerSearch,
    sortFor,
    sortingFor,
    type ContainerSearch,
} from "@/components/containers/container-search";
import { useContainerAction, useContainers, useContainersBulkAction, type ContainerRow } from "@/hooks/use-containers";
import { useTrackedOperations, isSettled } from "@/hooks/use-operation";
import { withIdempotencyKey } from "@/hooks/use-operations";
import { clockTime } from "@/lib/time";
import { containerRowId, type ContainerAction } from "@/types/containers";

export const Route = createFileRoute("/_authenticated/containers/")({
    validateSearch: (search: Record<string, unknown>): ContainerSearch => parseContainerSearch(search),
    component: ContainersPage,
});

const BULK_ACTIONS: { action: ContainerAction; label: string; icon: typeof RotateCw }[] = [
    { action: "restart", label: "RESTART", icon: RotateCw },
    { action: "stop", label: "STOP", icon: Square },
    { action: "start", label: "START", icon: Play },
    { action: "pause", label: "PAUSE", icon: Pause },
    { action: "unpause", label: "UNPAUSE", icon: Play },
    { action: "remove", label: "REMOVE", icon: Trash2 },
];

const plural = (count: number, one: string, many = `${one}s`) => `${count} ${count === 1 ? one : many}`;

function joinNames(names: string[]): string {
    return names.length <= 1 ? (names[0] ?? "") : `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
}

function ContainersPage() {
    const search = Route.useSearch();
    const navigate = Route.useNavigate();
    const containers = useContainers();
    const action = useContainerAction();
    const bulk = useContainersBulkAction();
    const tracked = useTrackedOperations("containers");
    const [selection, setSelection] = useState<RowSelectionState>({});
    const [removing, setRemoving] = useState<ContainerRow | null>(null);
    const [confirmBulk, setConfirmBulk] = useState<ContainerAction | null>(null);

    const snapshot = containers.data;
    const rows = snapshot?.items ?? [];
    const liveRows = rows.filter((row) => !row.stale);
    const nodes = snapshot?.nodes ?? [];
    const answered = nodes.filter((node) => node.status === "ok");
    const silent = nodes.filter((node) => node.status !== "ok");
    const filtered = applyFilters(rows, search);
    const selected = rows.filter((row) => selection[containerRowId(row)] && !row.stale);

    // Rows an in-flight operation acts on fade; rows a finished bulk failed on say so.
    const pending = new Set<string>();
    const failures = new Map<string, string>();
    for (const { operation, view } of tracked) {
        if (!view || !isSettled(view.state)) for (const target of operation.targets ?? []) pending.add(target);
        const results = view?.progress?.results;
        if (Array.isArray(results))
            for (const result of results as { nodeId?: string; containerId?: string; ok?: boolean; error?: string }[])
                if (result.ok === false && result.nodeId && result.containerId)
                    failures.set(`${result.nodeId}/${result.containerId}`, result.error ?? "failed");
    }

    function setSearch(next: ContainerSearch) {
        void navigate({ search: next, replace: true });
    }

    function start(row: ContainerRow, containerAction: ContainerAction, options: { force?: boolean } | null = null) {
        action.mutate(withIdempotencyKey({ nodeId: row.nodeId, containerId: row.containerId, action: containerAction, options, name: row.name }), {
            onError: (code) =>
                notify.error(`Couldn't ${containerAction} ${row.name}`, { code: String(code), message: "The controller did not accept the action." }),
        });
    }

    function startBulk(bulkAction: ContainerAction) {
        const targets = selected.map((row) => ({ nodeId: row.nodeId, containerId: row.containerId }));
        const force = bulkAction === "remove" && selected.some((row) => row.state === "running");
        bulk.mutate(withIdempotencyKey({ action: bulkAction, targets, force }), {
            onSuccess: () => setSelection({}),
            onError: (code) =>
                notify.error(`Couldn't ${bulkAction} ${plural(targets.length, "container")}`, { code: String(code), message: "Nothing was changed." }),
        });
    }

    const chips: FilterChip[] = [
        ...(search.state ?? []).map((state) => ({
            key: `state:${state}`,
            label: `status: ${state}`,
            onRemove: () => {
                const remaining = search.state?.filter((value) => value !== state) ?? [];
                setSearch({ ...search, state: remaining.length > 0 ? remaining : undefined });
            },
        })),
        ...(search.node ? [{ key: "node", label: `node: ${search.node}`, onRemove: () => setSearch({ ...search, node: undefined }) }] : []),
        ...(search.network ? [{ key: "network", label: `network: ${search.network}`, onRemove: () => setSearch({ ...search, network: undefined }) }] : []),
        ...(search.ports ? [{ key: "ports", label: "ports: published", onRemove: () => setSearch({ ...search, ports: undefined }) }] : []),
    ];
    const clearChips = () => setSearch({ q: search.q, sort: search.sort });

    const subtitle =
        silent.length > 0 && snapshot
            ? `showing ${liveRows.length} of ${rows.length} containers · ${answered.length} of ${nodes.length} nodes answered · snapshot ${clockTime(Date.parse(snapshot.snapshotAt), true)}`
            : `managing ${plural(liveRows.length, "container")} across ${plural(nodes.length, "node")}`;

    const filtering = activeFilterCount(search) > 0 || Boolean(search.q);
    const emptyState =
        rows.length === 0 && !filtering ? (
            <EmptyState
                icon={Box}
                title='No containers yet'
                primaryAction={{
                    label: "DEPLOY STACK",
                    restricted: { title: "Not built yet", reason: "Deploying stacks from Kontainer isn't part of this build." },
                }}
            >
                Containers show up here as soon as a stack or service schedules a task on one of your nodes.
            </EmptyState>
        ) : (
            <EmptyState
                title='No containers match these filters'
                primaryAction={{ label: "CLEAR FILTERS", onClick: () => setSearch({ sort: search.sort }) }}
                secondaryAction={{ label: `SHOW ALL ${rows.length}`, onClick: clearChips }}
            >
                Nothing matches {search.q ? `"${search.q}"` : "the search"}
                {chips.length > 0 ? ` with ${chips.map((chip) => chip.label).join(" · ")}` : ""}. Remove a filter, or clear them all.
            </EmptyState>
        );

    const errorState =
        containers.error && !snapshot ? (
            <ErrorState
                title="Couldn't load containers"
                details={{
                    code: String(containers.error),
                    message:
                        String(containers.error) === "too_many_containers"
                            ? "More than 2,000 containers; the cluster list is capped."
                            : "The controller answered, but the container inventory request failed. Nothing was changed.",
                }}
                onRetry={() => void containers.refetch()}
            />
        ) : undefined;

    const confirmCount = selected.length;
    return (
        <div className='flex flex-col gap-5'>
            <PageHeader title='CONTAINERS' subtitle={subtitle}>
                <ActionButton variant='outline' icon={RefreshCw} pending={containers.isFetching} onClick={() => void containers.refetch()}>
                    REFRESH
                </ActionButton>
            </PageHeader>
            {silent.length > 0 && snapshot && (
                <InlineAlert
                    tone='destructive'
                    title={`${joinNames(silent.map((node) => node.nodeId))} didn't answer — ${silent.length === 1 ? "its" : "their"} containers may be missing`}
                >
                    Partial results: {answered.length} of {nodes.length} nodes responded. Rows last reported by {joinNames(silent.map((node) => node.nodeId))}{" "}
                    are marked UNKNOWN until {silent.length === 1 ? "it checks" : "they check"} in again.
                </InlineAlert>
            )}
            <ContainerStatesStrip rows={rows} />
            <div className='flex flex-col gap-3'>
                {selected.length > 0 ? (
                    <BulkBar
                        count={selected.length}
                        total={liveRows.length}
                        context={`across ${plural(new Set(selected.map((row) => row.nodeId)).size, "node")}`}
                        onClear={() => setSelection({})}
                    >
                        {BULK_ACTIONS.map((entry) => (
                            <ActionButton
                                key={entry.action}
                                variant={entry.action === "remove" ? "destructive" : "outline"}
                                size='sm'
                                icon={entry.icon}
                                pending={bulk.isPending && bulk.variables?.action === entry.action}
                                onClick={() => (entry.action === "stop" || entry.action === "remove" ? setConfirmBulk(entry.action) : startBulk(entry.action))}
                            >
                                {entry.label}
                            </ActionButton>
                        ))}
                    </BulkBar>
                ) : (
                    <TableToolbar
                        search={search.q ?? ""}
                        onSearchChange={(q) => setSearch({ ...search, q: q || undefined })}
                        searchPlaceholder='Search containers'
                        filters={
                            <ContainerFilters
                                search={search}
                                nodes={nodes}
                                networks={[...new Set(rows.flatMap((row) => row.networks))].sort()}
                                onChange={setSearch}
                            />
                        }
                        chips={chips}
                        onClearFilters={clearChips}
                        sortOptions={CONTAINER_SORTS.map((value) => ({ value, label: SORT_LABELS[value] }))}
                        sort={search.sort ?? "recent"}
                        onSortChange={(sort) => setSearch({ ...search, sort: sort === "recent" ? undefined : (sort as ContainerSearch["sort"]) })}
                    />
                )}
                <PinnedOperations scope='containers' />
                <ContainersTable
                    rows={filtered}
                    snapshotAt={snapshot?.snapshotAt ?? new Date(0).toISOString()}
                    search={search.q ?? ""}
                    sorting={sortingFor(search.sort)}
                    onSortingChange={(sorting) => {
                        const sort = sortFor(sorting);
                        setSearch({ ...search, sort: sort === "recent" ? undefined : sort });
                    }}
                    selection={selection}
                    onSelectionChange={setSelection}
                    failures={failures}
                    pending={pending}
                    onAction={(row, containerAction) => start(row, containerAction)}
                    onRemove={setRemoving}
                    countLabel={filtered.length === rows.length && !search.q ? String(rows.length) : `${filtered.length} OF ${rows.length}`}
                    isFirstLoad={containers.isPending}
                    isRefetching={containers.isFetching && !containers.isPending}
                    emptyState={emptyState}
                    errorState={errorState}
                />
            </div>

            <ConfirmDialog
                open={removing !== null}
                onOpenChange={(open) => !open && setRemoving(null)}
                title={`Remove ${removing?.name ?? "container"}?`}
                description={
                    removing?.state === "running"
                        ? "It is running. Removing kills it first and deletes its writable layer. This cannot be undone."
                        : "Its writable layer is deleted. This cannot be undone."
                }
                confirmationText={removing?.state === "running" ? removing.name : undefined}
                confirmLabel='REMOVE'
                onConfirm={() => {
                    if (removing) start(removing, "remove", removing.state === "running" ? { force: true } : null);
                    setRemoving(null);
                }}
            />
            <ConfirmDialog
                open={confirmBulk !== null}
                onOpenChange={(open) => !open && setConfirmBulk(null)}
                title={`${confirmBulk === "remove" ? "Remove" : "Stop"} ${plural(confirmCount, "container")}?`}
                description={
                    confirmBulk === "remove" ? "Running ones are killed first. Removal cannot be undone." : "Each gets 10 seconds to stop before it is killed."
                }
                impact={selected
                    .slice(0, 8)
                    .map((row) => row.name)
                    .join(", ")
                    .concat(selected.length > 8 ? ` and ${selected.length - 8} more` : "")}
                confirmationText={String(confirmCount)}
                confirmLabel={confirmBulk === "remove" ? "REMOVE" : "STOP"}
                confirmIcon={confirmBulk === "remove" ? Trash2 : Square}
                onConfirm={() => {
                    if (confirmBulk) startBulk(confirmBulk);
                    setConfirmBulk(null);
                }}
            />
        </div>
    );
}
