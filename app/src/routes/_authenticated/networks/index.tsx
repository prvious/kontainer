import { useState } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { Network, Plus, RefreshCw } from "lucide-react";
import { PageHeader } from "@/components/page-header";
import { ActionButton } from "@/components/kit/action-button";
import { ConfirmDialog } from "@/components/kit/confirm-dialog";
import { EmptyState, ErrorState } from "@/components/kit/region-states";
import { InlineAlert } from "@/components/kit/inline-alert";
import { notify } from "@/components/kit/notify";
import { TableToolbar, type FilterChip } from "@/components/data-table/table-toolbar";
import { PinnedOperations } from "@/components/operations/pinned-operations";
import { NetworkOverviewStrip } from "@/components/networks/network-overview-strip";
import { NetworksTable } from "@/components/networks/networks-table";
import { NetworkFilters } from "@/components/networks/network-filters";
import { CreateNetworkDialog } from "@/components/networks/create-network-dialog";
import { activeNetworkFilterCount, applyNetworkFilters, driverSummary, parseNetworkSearch, type NetworkSearch } from "@/components/networks/network-search";
import { useNetworks, useRemoveNetwork } from "@/hooks/use-networks";
import { useNodes } from "@/hooks/use-nodes";
import { isSettled, useTrackedOperations } from "@/hooks/use-operation";
import { withIdempotencyKey } from "@/hooks/use-operations";
import type { NetworkView } from "@/types/networks";

export const Route = createFileRoute("/_authenticated/networks/")({
    validateSearch: (search: Record<string, unknown>): NetworkSearch => parseNetworkSearch(search),
    component: NetworksPage,
});

const HIGHLIGHT_MS = 3000;

function NetworksPage() {
    const search = Route.useSearch();
    const navigate = Route.useNavigate();
    const networks = useNetworks();
    const nodes = useNodes();
    const remove = useRemoveNetwork();
    const tracked = useTrackedOperations("networks");
    const [removing, setRemoving] = useState<NetworkView | null>(null);
    const [createOpen, setCreateOpen] = useState(false);

    const snapshot = networks.data;
    const rows = snapshot?.items ?? [];
    const outcomes = snapshot?.nodes ?? [];
    const silent = outcomes.filter((node) => node.status !== "ok");
    const filtered = applyNetworkFilters(rows, search);

    const pending = new Set<string>();
    for (const { operation, view } of tracked) if (!view || !isSettled(view.state)) for (const target of operation.targets ?? []) pending.add(target);

    /** Every navigation drops the highlight. */
    function setSearch(next: NetworkSearch) {
        void navigate({ search: { ...next, highlightedRow: undefined }, replace: true });
    }

    function created(row: string) {
        setCreateOpen(false);
        void navigate({ search: (previous: NetworkSearch) => ({ ...previous, highlightedRow: row }), replace: true });
        setTimeout(() => {
            void navigate({
                search: (previous: NetworkSearch) => (previous.highlightedRow === row ? { ...previous, highlightedRow: undefined } : previous),
                replace: true,
            });
        }, HIGHLIGHT_MS);
    }

    function confirmRemove(network: NetworkView) {
        remove.mutate(withIdempotencyKey({ id: network.id, scope: network.scope, nodeIds: network.nodeIds, name: network.name }), {
            onError: (code) =>
                notify.error(`Couldn't remove ${network.name}`, {
                    code: String(code),
                    message:
                        String(code) === "not_a_manager"
                            ? "The controller's Docker host is not a Swarm manager."
                            : "The controller did not accept the removal.",
                }),
        });
        setRemoving(null);
    }

    const chips: FilterChip[] = [
        ...(search.driver ? [{ key: "driver", label: `driver: ${search.driver}`, onRemove: () => setSearch({ ...search, driver: undefined }) }] : []),
        ...(search.scope ? [{ key: "scope", label: `scope: ${search.scope}`, onRemove: () => setSearch({ ...search, scope: undefined }) }] : []),
        ...(search.node ? [{ key: "node", label: `node: ${search.node}`, onRemove: () => setSearch({ ...search, node: undefined }) }] : []),
    ];
    const clearChips = () => setSearch({ q: search.q });
    const filtering = activeNetworkFilterCount(search) > 0 || Boolean(search.q);

    const emptyState =
        rows.length === 0 && !filtering ? (
            <EmptyState icon={Network} title='No networks yet' primaryAction={{ label: "CREATE NETWORK", icon: Plus, onClick: () => setCreateOpen(true) }}>
                Networks show up here as soon as the nodes report them.
            </EmptyState>
        ) : (
            <EmptyState
                title='No networks match these filters'
                primaryAction={{ label: "CLEAR FILTERS", onClick: () => setSearch({}) }}
                secondaryAction={{ label: `SHOW ALL ${rows.length}`, onClick: clearChips }}
            >
                Nothing matches {search.q ? `"${search.q}"` : "the search"}
                {chips.length > 0 ? ` with ${chips.map((chip) => chip.label).join(" · ")}` : ""}.
            </EmptyState>
        );

    const errorState =
        networks.error && !snapshot ? (
            <ErrorState
                title="Couldn't load networks"
                details={{ code: String(networks.error), message: "The controller answered, but the network list request failed. Nothing was changed." }}
                onRetry={() => void networks.refetch()}
            />
        ) : undefined;

    return (
        <div className='flex flex-col gap-5'>
            <PageHeader title='NETWORKS' subtitle={driverSummary(rows)}>
                <ActionButton variant='outline' icon={RefreshCw} pending={networks.isFetching} onClick={() => void networks.refetch()}>
                    REFRESH
                </ActionButton>
                <ActionButton variant='primary' icon={Plus} mutates onClick={() => setCreateOpen(true)}>
                    CREATE NETWORK
                </ActionButton>
            </PageHeader>
            {snapshot && snapshot.manager !== "ok" && (
                <InlineAlert
                    tone='warning'
                    title={snapshot.manager === "not_a_manager" ? "The controller isn't connected to a Swarm manager" : "The Swarm manager didn't answer"}
                >
                    Swarm networks are listed from the nodes that use them, and overlay networks can't be created or removed until the manager answers.
                </InlineAlert>
            )}
            {silent.length > 0 && snapshot && (
                <InlineAlert tone='destructive' title={`${silent.map((node) => node.nodeId).join(", ")} didn't answer — their networks may be missing`}>
                    Partial results: {outcomes.length - silent.length} of {outcomes.length} nodes responded.
                </InlineAlert>
            )}
            <NetworkOverviewStrip networks={rows} />
            <div className='flex flex-col gap-3'>
                <TableToolbar
                    search={search.q ?? ""}
                    onSearchChange={(q) => setSearch({ ...search, q: q || undefined })}
                    searchPlaceholder='Search networks by name or id'
                    filters={
                        <NetworkFilters
                            search={search}
                            drivers={[...new Set(rows.map((row) => row.driver))].sort()}
                            nodes={outcomes.map((node) => node.nodeId)}
                            onChange={setSearch}
                        />
                    }
                    chips={chips}
                    onClearFilters={clearChips}
                    sortOptions={[{ value: "name", label: "Name" }]}
                    sort='name'
                    onSortChange={() => {}}
                />
                <PinnedOperations scope='networks' />
                <NetworksTable
                    rows={filtered}
                    search={search.q ?? ""}
                    pending={pending}
                    highlighted={search.highlightedRow}
                    onRemove={setRemoving}
                    countLabel={filtered.length === rows.length && !search.q ? String(rows.length) : `${filtered.length} OF ${rows.length}`}
                    isFirstLoad={networks.isPending}
                    isRefetching={networks.isFetching && !networks.isPending}
                    emptyState={emptyState}
                    errorState={errorState}
                />
            </div>

            <ConfirmDialog
                open={removing !== null}
                onOpenChange={(open) => !open && setRemoving(null)}
                title={`Remove network ${removing?.name ?? ""}?`}
                description={
                    removing?.scope === "swarm"
                        ? "Nothing is attached. It is removed from the swarm; services can't join it afterwards."
                        : `Nothing is attached. It is removed from ${removing?.nodeIds[0] ?? "its node"}.`
                }
                confirmLabel='REMOVE'
                onConfirm={() => {
                    if (removing) confirmRemove(removing);
                }}
            />
            <CreateNetworkDialog open={createOpen} onOpenChange={setCreateOpen} nodes={nodes.data?.items ?? []} networks={rows} onCreated={created} />
        </div>
    );
}
