import { useState } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { SortingState } from "@tanstack/react-table";
import { BookOpen, Layers, Upload } from "lucide-react";
import { PageHeader } from "@/components/page-header";
import { ActionButton } from "@/components/kit/action-button";
import { EmptyState, ErrorState } from "@/components/kit/region-states";
import { StaleSnapshotAlert } from "@/components/kit/stale-snapshot-alert";
import { TableToolbar } from "@/components/data-table/table-toolbar";
import { PinnedOperations } from "@/components/operations/pinned-operations";
import { StacksTable } from "@/components/stacks/stacks-table";
import { RemoveStackDialog } from "@/components/stacks/remove-stack-dialog";
import { useNow } from "@/hooks/use-now";
import { isSettled, useTrackedOperations } from "@/hooks/use-operation";
import { useStacks } from "@/hooks/use-stacks";

/** `?q=&sort=` validated with defaults. */
interface StackSearch {
    q?: string;
    sort?: "deployed";
}

export const Route = createFileRoute("/_authenticated/stacks/")({
    validateSearch: (search: Record<string, unknown>): StackSearch => ({
        ...(typeof search.q === "string" && search.q.trim() !== "" ? { q: search.q.slice(0, 200) } : {}),
        ...(search.sort === "deployed" ? { sort: "deployed" as const } : {}),
    }),
    component: StacksPage,
});

const DOCS_URL = "https://docs.docker.com/engine/swarm/stack-deploy/";
const NOT_BUILT = { title: "Not built yet", reason: "Deploying stacks from Kontainer isn't part of this build; use docker stack deploy." };
const plural = (count: number, one: string) => `${count} ${one}${count === 1 ? "" : "s"}`;

function StacksPage() {
    const search = Route.useSearch();
    const navigate = Route.useNavigate();
    const stacks = useStacks();
    const tracked = useTrackedOperations("stacks");
    const now = useNow();
    const [removing, setRemoving] = useState<string | null>(null);

    const rows = stacks.data?.items ?? [];
    const pending = new Set<string>();
    for (const { operation, view } of tracked) if (!view || !isSettled(view.state)) for (const target of operation.targets ?? []) pending.add(target);
    const services = rows.reduce((total, row) => total + row.services, 0);
    const containers = rows.reduce((total, row) => total + row.runningContainers, 0);
    const errorCode = stacks.error ? String(stacks.error) : null;
    const sorting: SortingState = search.sort === "deployed" ? [{ id: "lastDeployed", desc: true }] : [{ id: "name", desc: false }];

    function setSearch(next: StackSearch) {
        void navigate({ search: next, replace: true });
    }

    const emptyState =
        rows.length === 0 && !search.q ? (
            <EmptyState
                icon={Layers}
                title='No stacks deployed'
                primaryAction={{ label: "DEPLOY STACK", icon: Upload, restricted: NOT_BUILT }}
                secondaryAction={{ label: "DOCKER DOCS", icon: BookOpen, onClick: () => void openUrl(DOCS_URL).catch(() => {}) }}
            >
                A stack is a Compose file deployed with docker stack deploy. Its services, containers, and health appear here as soon as it runs.
            </EmptyState>
        ) : (
            <EmptyState title='No stacks match this search' primaryAction={{ label: "CLEAR SEARCH", onClick: () => setSearch({ sort: search.sort }) }}>
                Nothing matches "{search.q}".
            </EmptyState>
        );

    const retryIn = Math.max(0, Math.ceil((stacks.errorUpdatedAt + 10_000 - now) / 1000));
    // Like Services: quorum loss replaces the rows; any other failed refetch keeps the snapshot, marked stale.
    const errorState =
        errorCode && (errorCode === "quorum_unavailable" || !stacks.data) ? (
            <ErrorState
                title="Couldn't load stacks"
                details={{ status: 503, code: errorCode, message: "The stack list request failed." }}
                retryLabel={retryIn > 0 ? `RETRY IN ${retryIn}s` : "TRY AGAIN"}
                onRetry={() => void stacks.refetch()}
            >
                {errorCode === "quorum_unavailable"
                    ? "The Swarm manager can't reach a majority of managers, so stacks can't be read. Your workloads keep running — nothing here is reported as down while the cluster can't answer."
                    : "The controller answered, but the stack list request failed. Nothing was changed."}
            </ErrorState>
        ) : undefined;

    const empty = rows.length === 0 && !search.q && !errorCode && !stacks.isPending;
    return (
        <div className='flex flex-col gap-5'>
            <PageHeader
                title='STACKS'
                subtitle={`${plural(rows.length, "stack")} deployed · ${plural(services, "service")} · ${plural(containers, "container")}`}
            >
                <ActionButton variant='primary' icon={Upload} restricted={NOT_BUILT}>
                    DEPLOY STACK
                </ActionButton>
            </PageHeader>
            <PinnedOperations scope='stacks' />
            <div className='flex flex-col gap-3'>
                {!empty && (
                    <TableToolbar
                        search={search.q ?? ""}
                        onSearchChange={(q) => setSearch({ ...search, q: q || undefined })}
                        searchPlaceholder='Search stacks'
                        sortOptions={[
                            { value: "name", label: "Name" },
                            { value: "deployed", label: "Last deployed" },
                        ]}
                        sort={search.sort ?? "name"}
                        onSortChange={(sort) => setSearch({ ...search, sort: sort === "deployed" ? "deployed" : undefined })}
                    />
                )}
                <StacksTable
                    rows={rows}
                    search={search.q ?? ""}
                    sorting={sorting}
                    onSortingChange={(next) => setSearch({ ...search, sort: next[0]?.id === "lastDeployed" ? "deployed" : undefined })}
                    pending={pending}
                    onShowServices={(stack) => void navigate({ to: "/services", search: { stack: stack.name } })}
                    onRemove={(stack) => setRemoving(stack.name)}
                    writable={!stacks.isError}
                    isFirstLoad={stacks.isPending}
                    isRefetching={stacks.isFetching && !stacks.isPending}
                    stale={stacks.error && stacks.data ? <StaleSnapshotAlert query={stacks} /> : undefined}
                    emptyState={emptyState}
                    errorState={errorState}
                />
            </div>
            <RemoveStackDialog name={removing} onClose={() => setRemoving(null)} />
        </div>
    );
}
