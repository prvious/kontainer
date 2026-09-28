import { useState } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { ArrowDownUp, HardDrive, Plus, RefreshCw } from "lucide-react";
import { PageHeader } from "@/components/page-header";
import { ActionButton } from "@/components/kit/action-button";
import { ConfirmDialog } from "@/components/kit/confirm-dialog";
import { EmptyState, ErrorState } from "@/components/kit/region-states";
import { InlineAlert } from "@/components/kit/inline-alert";
import { notify } from "@/components/kit/notify";
import { TableToolbar, type FilterChip } from "@/components/data-table/table-toolbar";
import { PinnedOperations } from "@/components/operations/pinned-operations";
import { bytes } from "@/components/containers/container-format";
import { VolumeStorageStrip } from "@/components/volumes/volume-storage-strip";
import { VolumesTable } from "@/components/volumes/volumes-table";
import { VolumeFilters } from "@/components/volumes/volume-filters";
import { CreateVolumeDialog } from "@/components/volumes/create-volume-dialog";
import {
    VOLUME_SORTS,
    VOLUME_SORT_LABELS,
    activeVolumeFilterCount,
    allocated,
    applyVolumeFilters,
    parseVolumeSearch,
    volumeSortFor,
    volumeSortingFor,
    type VolumeSearch,
} from "@/components/volumes/volume-search";
import { useNodes } from "@/hooks/use-nodes";
import { isSettled, useTrackedOperations } from "@/hooks/use-operation";
import { withIdempotencyKey } from "@/hooks/use-operations";
import { useRemoveVolume, useVolumes } from "@/hooks/use-volumes";
import { volumeRowId, type VolumeView } from "@/types/volumes";

export const Route = createFileRoute("/_authenticated/volumes/")({
    validateSearch: (search: Record<string, unknown>): VolumeSearch => parseVolumeSearch(search),
    component: VolumesPage,
});

const HIGHLIGHT_MS = 3000;
const plural = (count: number, one: string, many = `${one}s`) => `${count} ${count === 1 ? one : many}`;

function VolumesPage() {
    const search = Route.useSearch();
    const navigate = Route.useNavigate();
    const volumes = useVolumes();
    const nodes = useNodes();
    const remove = useRemoveVolume();
    const tracked = useTrackedOperations("volumes");
    const [removing, setRemoving] = useState<VolumeView | null>(null);
    const [createOpen, setCreateOpen] = useState(false);

    const snapshot = volumes.data;
    const rows = snapshot?.items ?? [];
    const outcomes = snapshot?.nodes ?? [];
    const silent = outcomes.filter((node) => node.status !== "ok");
    const filtered = applyVolumeFilters(rows, search);
    const total = allocated(rows);

    const pending = new Set<string>();
    for (const { operation, view } of tracked) if (!view || !isSettled(view.state)) for (const target of operation.targets ?? []) pending.add(target);

    /** Every navigation drops the highlight. */
    function setSearch(next: VolumeSearch) {
        void navigate({ search: { ...next, highlightedRow: undefined }, replace: true });
    }

    function created(volume: { nodeId: string; name: string }) {
        setCreateOpen(false);
        const row = volumeRowId(volume);
        void navigate({ search: (previous: VolumeSearch) => ({ ...previous, highlightedRow: row }), replace: true });
        setTimeout(() => {
            void navigate({
                search: (previous: VolumeSearch) => (previous.highlightedRow === row ? { ...previous, highlightedRow: undefined } : previous),
                replace: true,
            });
        }, HIGHLIGHT_MS);
    }

    function confirmRemove(volume: VolumeView) {
        remove.mutate(withIdempotencyKey({ nodeId: volume.nodeId, name: volume.name }), {
            onError: (code) =>
                notify.error(`Couldn't remove ${volume.name}`, {
                    code: String(code),
                    message: "The controller did not accept the removal. Nothing was changed.",
                }),
        });
        setRemoving(null);
    }

    const chips: FilterChip[] = [
        ...(search.node ? [{ key: "node", label: `node: ${search.node}`, onRemove: () => setSearch({ ...search, node: undefined }) }] : []),
        ...(search.driver ? [{ key: "driver", label: `driver: ${search.driver}`, onRemove: () => setSearch({ ...search, driver: undefined }) }] : []),
        ...(search.attached
            ? [{ key: "attached", label: search.attached === "yes" ? "attached" : "unattached", onRemove: () => setSearch({ ...search, attached: undefined }) }]
            : []),
    ];
    const clearChips = () => setSearch({ q: search.q, sort: search.sort });
    const filtering = activeVolumeFilterCount(search) > 0 || Boolean(search.q);

    const emptyState =
        rows.length === 0 && !filtering ? (
            <EmptyState icon={HardDrive} title='No volumes yet' primaryAction={{ label: "CREATE VOLUME", icon: Plus, onClick: () => setCreateOpen(true) }}>
                Volumes keep data outside a container's writable layer. Create one here, or let a stack declare it.
            </EmptyState>
        ) : (
            <EmptyState
                title='No volumes match these filters'
                primaryAction={{ label: "CLEAR FILTERS", onClick: () => setSearch({ sort: search.sort }) }}
                secondaryAction={{ label: `SHOW ALL ${rows.length}`, onClick: clearChips }}
            >
                Nothing matches {search.q ? `"${search.q}"` : "the search"}
                {chips.length > 0 ? ` with ${chips.map((chip) => chip.label).join(" · ")}` : ""}.
            </EmptyState>
        );

    const errorState =
        volumes.error && !snapshot ? (
            <ErrorState
                title="Couldn't load volumes"
                details={{ code: String(volumes.error), message: "The controller answered, but the volume list request failed. Nothing was changed." }}
                onRetry={() => void volumes.refetch()}
            />
        ) : undefined;

    const sort = search.sort ?? "size";
    const subtitle = `${plural(rows.length, "volume")} · ${bytes(total.bytes)} allocated${total.pending ? " · sizes pending" : ""}`;
    return (
        <div className='flex flex-col gap-5'>
            <PageHeader title='VOLUMES' subtitle={subtitle}>
                <ActionButton variant='outline' icon={RefreshCw} pending={volumes.isFetching} onClick={() => void volumes.refetch()}>
                    REFRESH
                </ActionButton>
                <ActionButton variant='primary' icon={Plus} onClick={() => setCreateOpen(true)}>
                    CREATE VOLUME
                </ActionButton>
            </PageHeader>
            {silent.length > 0 && snapshot && (
                <InlineAlert tone='destructive' title={`${silent.map((node) => node.nodeId).join(", ")} didn't answer — their volumes may be missing`}>
                    Partial results: {outcomes.length - silent.length} of {outcomes.length} nodes responded.
                </InlineAlert>
            )}
            <VolumeStorageStrip volumes={rows} />
            <div className='flex flex-col gap-3'>
                <TableToolbar
                    search={search.q ?? ""}
                    onSearchChange={(q) => setSearch({ ...search, q: q || undefined })}
                    searchPlaceholder='Search volumes or mount points'
                    filters={
                        <VolumeFilters
                            search={search}
                            nodes={outcomes.map((node) => node.nodeId)}
                            drivers={[...new Set(rows.map((row) => row.driver))].sort()}
                            onChange={setSearch}
                        />
                    }
                    chips={chips}
                    onClearFilters={clearChips}
                    sortOptions={VOLUME_SORTS.map((value) => ({ value, label: VOLUME_SORT_LABELS[value] }))}
                    sort={sort}
                    onSortChange={(next) => setSearch({ ...search, sort: next === "size" ? undefined : (next as VolumeSearch["sort"]) })}
                />
                <PinnedOperations scope='volumes' />
                <VolumesTable
                    rows={filtered}
                    search={search.q ?? ""}
                    sorting={volumeSortingFor(search.sort)}
                    onSortingChange={(sorting) => {
                        const next = volumeSortFor(sorting);
                        setSearch({ ...search, sort: next === "size" ? undefined : next });
                    }}
                    pending={pending}
                    highlighted={search.highlightedRow}
                    onRemove={setRemoving}
                    countLabel={filtered.length === rows.length && !search.q ? String(rows.length) : `${filtered.length} OF ${rows.length}`}
                    headerRight={
                        <span className='flex items-center gap-1.5 font-mono text-3xs font-bold tracking-[1px] text-muted-foreground uppercase'>
                            <ArrowDownUp aria-hidden className='size-3' /> {VOLUME_SORT_LABELS[sort]} {sort === "name" ? "asc" : "desc"}
                        </span>
                    }
                    isFirstLoad={volumes.isPending}
                    isRefetching={volumes.isFetching && !volumes.isPending}
                    emptyState={emptyState}
                    errorState={errorState}
                />
            </div>

            <ConfirmDialog
                open={removing !== null}
                onOpenChange={(open) => !open && setRemoving(null)}
                title={`Remove volume ${removing?.name ?? ""}?`}
                description={`Its data on ${removing?.nodeId ?? "the node"} is deleted. This cannot be undone.`}
                confirmationText={removing?.name}
                confirmLabel='REMOVE'
                onConfirm={() => {
                    if (removing) confirmRemove(removing);
                }}
            />
            <CreateVolumeDialog open={createOpen} onOpenChange={setCreateOpen} nodes={nodes.data?.items ?? []} onCreated={created} />
        </div>
    );
}
