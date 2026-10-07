/**
 * Swarm services crossing the Tauri boundary.
 *
 * Mirrors `app/src-tauri/src/session/manager_services.rs`; the shared fixture at
 * `app/tests/fixtures/services-ipc.json` keeps both sides in sync.
 */

export const SERVICE_MODES = ["replicated", "global"] as const;
export type ServiceMode = (typeof SERVICE_MODES)[number];

export interface ServiceView {
    id: string;
    name: string;
    /** Without the digest. */
    image: string;
    imageReference: string;
    mode: ServiceMode;
    /** Replicated: the replicas asked for. Global: one per eligible node. */
    desiredReplicas: number;
    runningReplicas: number;
    /** `published:target/proto`. */
    ports: string[];
    createdAt: string | null;
    updatedAt: string | null;
    /** Swarm's object version; writes send it back. */
    version: number;
    stack: string | null;
    updateState: string | null;
}

export interface ServiceListView {
    items: ServiceView[];
    snapshotAt: string;
}

/** What the replica tooltip reads of one service. */
export interface ServiceDetailView {
    latestError: string | null;
}

export type ServiceStatus = "running" | "updating" | "scaling" | "partial" | "stopped";

/**
 * The STATUS column: `scaling` while a scale operation of ours runs, `updating` while Swarm
 * updates it or another operation runs, then `stopped` (nothing running), `partial`
 * (0 < running < desired), or `running` (converged).
 */
export function serviceStatus(
    service: Pick<ServiceView, "runningReplicas" | "desiredReplicas" | "updateState">,
    inFlight: "scale" | "other" | null,
): ServiceStatus {
    if (inFlight === "scale") return "scaling";
    if (inFlight === "other" || service.updateState === "updating" || service.updateState === "rollback_started") return "updating";
    if (service.runningReplicas === 0 && service.desiredReplicas > 0) return "stopped";
    if (service.runningReplicas < service.desiredReplicas) return "partial";
    return "running";
}
