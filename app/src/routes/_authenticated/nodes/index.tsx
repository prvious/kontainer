import { createFileRoute } from "@tanstack/react-router";
import { Plus, Server } from "lucide-react";
import { PageHeader } from "@/components/page-header";
import { ActionButton } from "@/components/kit/action-button";
import { EmptyState, ErrorState } from "@/components/kit/region-states";
import { SkeletonStat } from "@/components/kit/skeletons";
import { StaleSnapshotAlert } from "@/components/kit/stale-snapshot-alert";
import { ClusterLoadStrip } from "@/components/nodes/cluster-load-strip";
import { NodeCard } from "@/components/nodes/node-card";
import { useNow } from "@/hooks/use-now";
import { useClusterSummary, useNodes, useSamples } from "@/hooks/use-nodes";
import { cn } from "@/lib/utils";

export const Route = createFileRoute("/_authenticated/nodes/")({
    component: NodesPage,
});

/** Enrollment (04.S6) is G17: the ADD NODE dialog is not built. */
const NOT_BUILT = { title: "Not built yet", reason: "Adding a node from Kontainer isn't part of this build; start an agent on the host instead." };
const plural = (count: number, one: string) => `${count} ${one}${count === 1 ? "" : "s"}`;

function NodesPage() {
    const nodes = useNodes();
    const summary = useClusterSummary();
    const samples = useSamples("cluster", "5m", "5s");
    const now = useNow();

    const items = nodes.data?.items ?? [];
    const counts = summary.data;
    const subtitle = counts
        ? [
              plural(counts.nodesTotal, "node"),
              `${counts.nodesActive} active`,
              ...(counts.nodesDegraded > 0 ? [`${counts.nodesDegraded} degraded`] : []),
              `${counts.nodesDown} down`,
              "Docker Swarm cluster",
          ].join(" · ")
        : "Docker Swarm cluster";

    let grid;
    if (nodes.isPending) {
        grid = (
            <div aria-label='Loading nodes' aria-busy className='grid grid-cols-2 gap-3 xl:grid-cols-3'>
                {[0, 1, 2].map((index) => (
                    <SkeletonStat key={index} className='h-66' />
                ))}
            </div>
        );
    } else if (nodes.error && !nodes.data) {
        grid = (
            <ErrorState
                title="Couldn't load nodes"
                details={{ status: 503, code: String(nodes.error), message: "The node list request failed." }}
                onRetry={() => void nodes.refetch()}
            >
                The controller answered, but node membership couldn't be read. Nothing was changed.
            </ErrorState>
        );
    } else if (items.length === 0) {
        grid = (
            <EmptyState icon={Server} title='No nodes yet' primaryAction={{ label: "ADD NODE", icon: Plus, restricted: NOT_BUILT }}>
                Nodes appear here once their agent checks in with the controller.
            </EmptyState>
        );
    } else {
        // A failed refetch keeps the last cards, faded under the stale banner.
        grid = (
            <div className={cn("grid grid-cols-2 gap-3 xl:grid-cols-3", nodes.error && "opacity-50")}>
                {items.map((node) => (
                    <NodeCard key={node.id} node={node} now={now} />
                ))}
            </div>
        );
    }

    return (
        <div className='flex flex-col gap-6'>
            <PageHeader title='NODES' subtitle={subtitle}>
                <ActionButton variant='primary' icon={Plus} restricted={NOT_BUILT}>
                    ADD NODE
                </ActionButton>
            </PageHeader>
            <ClusterLoadStrip summary={summary.data} points={samples.data?.points ?? []} now={now} />
            {nodes.error && nodes.data && <StaleSnapshotAlert query={nodes} />}
            {grid}
        </div>
    );
}
