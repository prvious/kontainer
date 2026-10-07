import { bytes } from "@/components/containers/container-format";
import { isSettled } from "@/hooks/use-operation";
import { nodePullsFromProgress, type NodePull } from "@/types/images";
import type { OperationView } from "@/types/operations";

export const PRIVATE_REGISTRY_HINT = "Private registries need credentials, not built yet.";

/** One pull this dialog started: the first for every chosen node, then one per RETRY. */
export interface PullAttempt {
    operationId: string;
    nodeIds: string[];
}

export type NodeState = "queued" | NodePull["status"] | "cancelled";

export interface NodeProgress {
    nodeId: string;
    state: NodeState;
    pull: NodePull | null;
    error: string | null;
}

/** Each node's latest attempt decides its row: a retry replaces the failure it retries. */
export function nodeProgress(attempts: PullAttempt[], views: (OperationView | undefined)[]): NodeProgress[] {
    const order = [...new Set(attempts.flatMap((attempt) => attempt.nodeIds))];
    return order.map((nodeId) => {
        const index = attempts.reduce((latest, attempt, position) => (attempt.nodeIds.includes(nodeId) ? position : latest), -1);
        const view = views[index];
        const pull = nodePullsFromProgress(view?.progress).find((entry) => entry.nodeId === nodeId) ?? null;
        if (view?.state === "cancelled") return { nodeId, state: "cancelled", pull, error: null };
        if (pull && (pull.status !== "pulling" || !view || !isSettled(view.state))) return { nodeId, state: pull.status, pull, error: pull.error ?? null };
        if (view && isSettled(view.state))
            return { nodeId, state: "failed", pull, error: view.errorMessage ?? "The pull ended without a result for this node." };
        return { nodeId, state: view?.state === "running" ? "pulling" : "queued", pull, error: null };
    });
}

export function percent(pull: NodePull | null, state: NodeState): number {
    if (state === "done") return 100;
    if (!pull) return 0;
    if (pull.totalBytes > 0) return Math.min(100, Math.round((pull.bytes / pull.totalBytes) * 100));
    return pull.layersTotal > 0 ? Math.round((pull.layersDone / pull.layersTotal) * 100) : 0;
}

export function detail(progress: NodeProgress): string {
    const { pull, state } = progress;
    if (state === "queued") return "waiting to start";
    if (state === "cancelled") return "cancelled";
    if (state === "failed") return "failed";
    if (state === "done") return pull && pull.totalBytes > 0 ? `pulled · ${bytes(pull.totalBytes)}` : "pulled";
    if (!pull || pull.layersTotal === 0) return "resolving";
    const sizes = pull.totalBytes > 0 ? ` · ${bytes(pull.bytes)}/${bytes(pull.totalBytes)}` : "";
    return `layer ${pull.layersDone}/${pull.layersTotal} · ${percent(pull, state)}%${sizes}`;
}
