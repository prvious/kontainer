import { useState } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { Activity, Plus, RotateCcw, RotateCw } from "lucide-react";
import { PageHeader } from "@/components/page-header";
import { ActionButton } from "@/components/kit/action-button";
import { ConfirmDialog } from "@/components/kit/confirm-dialog";
import { EmptyState, ErrorState } from "@/components/kit/region-states";
import { notify } from "@/components/kit/notify";
import { TableToolbar, type FilterChip } from "@/components/data-table/table-toolbar";
import { PinnedOperations } from "@/components/operations/pinned-operations";
import { ServicesTable } from "@/components/services/services-table";
import { ServiceFilters } from "@/components/services/service-filters";
import {
    SERVICE_SORTS,
    SERVICE_SORT_LABELS,
    activeServiceFilterCount,
    applyServiceFilters,
    parseServiceSearch,
    serviceSortFor,
    serviceSortingFor,
    serviceSummary,
    type ServiceSearch,
} from "@/components/services/service-search";
import { useNow } from "@/hooks/use-now";
import { isSettled, useTrackedOperations } from "@/hooks/use-operation";
import { withIdempotencyKey } from "@/hooks/use-operations";
import { QUORUM_RETRY_MS, useRestartService, useRollbackService, useScaleService, useServices } from "@/hooks/use-services";
import { clockTime, relativeTime } from "@/lib/time";
import { serviceStatus, type ServiceView } from "@/types/services";

export const Route = createFileRoute("/_authenticated/services/")({
    validateSearch: (search: Record<string, unknown>): ServiceSearch => parseServiceSearch(search),
    component: ServicesPage,
});

const NOT_BUILT = { title: "Not built yet", reason: "Creating services from Kontainer isn't part of this build." };
const PAUSED = { title: "Changes are paused", reason: "Writes need manager quorum. Create and scale come back once the managers are reachable." };

type Pending = { kind: "restart" | "rollback"; service: ServiceView } | null;

function ServicesPage() {
    const search = Route.useSearch();
    const navigate = Route.useNavigate();
    const services = useServices();
    const scale = useScaleService();
    const restart = useRestartService();
    const rollback = useRollbackService();
    const tracked = useTrackedOperations("services");
    const now = useNow();
    const [editing, setEditing] = useState<string | null>(null);
    const [confirming, setConfirming] = useState<Pending>(null);

    const snapshot = services.data;
    const rows = snapshot?.items ?? [];
    const errorCode = services.error ? String(services.error) : null;
    const quorumLost = errorCode === "quorum_unavailable";
    const writable = !services.isError;

    // Operations of ours still running, by service.
    const scaling = new Map<string, { desired: number; running: number } | null>();
    const updating = new Set<string>();
    for (const { operation, view } of tracked) {
        if (view && isSettled(view.state)) continue;
        for (const target of operation.targets ?? []) {
            const [kind, id] = target.split(":");
            if (!id) continue;
            if (kind === "scale") {
                const progress = view?.progress;
                scaling.set(
                    id,
                    progress && typeof progress.desired === "number" && typeof progress.running === "number"
                        ? { desired: progress.desired, running: progress.running }
                        : null,
                );
            } else updating.add(id);
        }
    }
    const statusOf = (service: ServiceView) => serviceStatus(service, scaling.has(service.id) ? "scale" : updating.has(service.id) ? "other" : null);
    const scalingOf = (service: ServiceView) =>
        scaling.has(service.id) ? (scaling.get(service.id) ?? { desired: service.desiredReplicas, running: service.runningReplicas }) : null;
    const filtered = applyServiceFilters(rows, search, statusOf);

    function setSearch(next: ServiceSearch) {
        void navigate({ search: next, replace: true });
    }

    function refused(title: string) {
        return (code: unknown) => {
            const reason = String(code);
            if (reason === "version_conflict") void services.refetch();
            notify.error(title, {
                code: reason,
                message:
                    reason === "version_conflict"
                        ? "The service changed since this page read it. The list was refreshed; try again."
                        : reason === "quorum_unavailable"
                          ? "The swarm has no quorum, so nothing was changed."
                          : "The controller did not accept the change.",
            });
        };
    }

    function submitScale(service: ServiceView, replicas: number) {
        setEditing(null);
        if (replicas === service.desiredReplicas) return;
        scale.mutate(withIdempotencyKey({ serviceId: service.id, name: service.name, version: service.version, replicas, from: service.desiredReplicas }), {
            onError: refused(`Couldn't scale ${service.name}`),
        });
    }

    function confirmAction(pending: NonNullable<Pending>) {
        const variables = withIdempotencyKey({ serviceId: pending.service.id, name: pending.service.name, version: pending.service.version });
        if (pending.kind === "restart") restart.mutate(variables, { onError: refused(`Couldn't restart ${pending.service.name}`) });
        else rollback.mutate(variables, { onError: refused(`Couldn't roll back ${pending.service.name}`) });
        setConfirming(null);
    }

    const chips: FilterChip[] = [
        ...(search.mode ? [{ key: "mode", label: `mode: ${search.mode}`, onRemove: () => setSearch({ ...search, mode: undefined }) }] : []),
        ...(search.stack ? [{ key: "stack", label: `stack: ${search.stack}`, onRemove: () => setSearch({ ...search, stack: undefined }) }] : []),
        ...(search.status ? [{ key: "status", label: `status: ${search.status}`, onRemove: () => setSearch({ ...search, status: undefined }) }] : []),
    ];
    const clearChips = () => setSearch({ q: search.q, sort: search.sort });
    const filtering = activeServiceFilterCount(search) > 0 || Boolean(search.q);

    const emptyState =
        rows.length === 0 && !filtering ? (
            <EmptyState icon={Activity} title='No services yet' primaryAction={{ label: "CREATE SERVICE", icon: Plus, restricted: NOT_BUILT }}>
                Services show up here once a stack is deployed or a service is created with the Docker CLI.
            </EmptyState>
        ) : (
            <EmptyState
                title='No services match these filters'
                primaryAction={{ label: "CLEAR FILTERS", onClick: () => setSearch({ sort: search.sort }) }}
                secondaryAction={{ label: `SHOW ALL ${rows.length}`, onClick: clearChips }}
            >
                Nothing matches {search.q ? `"${search.q}"` : "the search"}
                {chips.length > 0 ? ` with ${chips.map((chip) => chip.label).join(" · ")}` : ""}.
            </EmptyState>
        );

    const retryIn = Math.max(0, Math.ceil((services.errorUpdatedAt + QUORUM_RETRY_MS - now) / 1000));
    const errorState = errorCode ? (
        <ErrorState
            title="Couldn't load services"
            details={{
                status: 503,
                code: errorCode,
                message: quorumLost ? "The swarm has no quorum." : "The service list request failed.",
            }}
            retryLabel={retryIn > 0 ? `RETRY IN ${retryIn}s` : "TRY AGAIN"}
            onRetry={() => void services.refetch()}
        >
            {quorumLost
                ? "The Swarm manager can't reach a majority of managers, so the service list can't be read. Your workloads keep running — nothing here is reported as down while the cluster can't answer."
                : errorCode === "not_a_manager"
                  ? "The controller's Docker host is not a Swarm manager, so there are no services to show."
                  : "The controller answered, but the service list request failed. Nothing was changed."}
        </ErrorState>
    ) : undefined;

    const subtitle =
        services.isError && snapshot
            ? `last known: ${rows.length} ${rows.length === 1 ? "service" : "services"} · snapshot ${clockTime(Date.parse(snapshot.snapshotAt), true)} (${relativeTime(Date.parse(snapshot.snapshotAt), now)})`
            : serviceSummary(rows);

    return (
        <div className='flex flex-col gap-5'>
            <PageHeader title='SERVICES' subtitle={subtitle}>
                <ActionButton variant='primary' icon={Plus} restricted={writable ? NOT_BUILT : PAUSED}>
                    CREATE SERVICE
                </ActionButton>
            </PageHeader>
            <PinnedOperations scope='services' />
            <div className='flex flex-col gap-3'>
                <TableToolbar
                    search={search.q ?? ""}
                    onSearchChange={(q) => setSearch({ ...search, q: q || undefined })}
                    searchPlaceholder='Search services by name or image'
                    filters={
                        <ServiceFilters
                            search={search}
                            stacks={[...new Set(rows.flatMap((row) => (row.stack ? [row.stack] : [])))].sort()}
                            onChange={setSearch}
                        />
                    }
                    chips={chips}
                    onClearFilters={clearChips}
                    sortOptions={SERVICE_SORTS.map((value) => ({ value, label: SERVICE_SORT_LABELS[value] }))}
                    sort={search.sort ?? "name"}
                    onSortChange={(sort) => setSearch({ ...search, sort: sort === "updated" ? "updated" : undefined })}
                />
                <ServicesTable
                    rows={filtered}
                    search={search.q ?? ""}
                    sorting={serviceSortingFor(search.sort)}
                    onSortingChange={(sorting) => {
                        const sort = serviceSortFor(sorting);
                        setSearch({ ...search, sort: sort === "updated" ? "updated" : undefined });
                    }}
                    statusOf={statusOf}
                    scalingOf={scalingOf}
                    editing={editing}
                    onEdit={setEditing}
                    onScale={submitScale}
                    onRestart={(service) => setConfirming({ kind: "restart", service })}
                    onRollback={(service) => setConfirming({ kind: "rollback", service })}
                    writable={writable}
                    countLabel={errorCode ? "—" : filtered.length === rows.length && !search.q ? String(rows.length) : `${filtered.length} OF ${rows.length}`}
                    isFirstLoad={services.isPending}
                    isRefetching={services.isFetching && !services.isPending}
                    emptyState={emptyState}
                    errorState={errorState}
                />
            </div>

            <ConfirmDialog
                open={confirming !== null}
                onOpenChange={(open) => !open && setConfirming(null)}
                tone='neutral'
                icon={confirming?.kind === "rollback" ? RotateCcw : RotateCw}
                confirmIcon={confirming?.kind === "rollback" ? RotateCcw : RotateCw}
                title={confirming?.kind === "rollback" ? `Roll back ${confirming.service.name}?` : `Restart ${confirming?.service.name ?? ""}?`}
                description={
                    confirming?.kind === "rollback"
                        ? "Swarm returns it to its previous spec, replacing tasks as its update settings allow."
                        : "Every task is replaced, as its update settings allow. Traffic keeps flowing if it has more than one replica."
                }
                confirmLabel={confirming?.kind === "rollback" ? "ROLL BACK" : "RESTART"}
                onConfirm={() => {
                    if (confirming) confirmAction(confirming);
                }}
            />
        </div>
    );
}
