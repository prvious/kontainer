/**
 * Stacks crossing the Tauri boundary.
 *
 * Mirrors `app/src-tauri/src/session/manager_stacks.rs`; the shared fixture at
 * `app/tests/fixtures/stacks-ipc.json` keeps both sides in sync.
 */

export const STACK_STATUSES = ["running", "partial", "stopped"] as const;
export type StackStatus = (typeof STACK_STATUSES)[number];

export interface StackView {
    name: string;
    status: StackStatus;
    services: number;
    /** Tasks running and desired, summed over the stack's services. */
    runningContainers: number;
    desiredContainers: number;
    cpuPercent: number | null;
    memoryBytes: number | null;
    lastDeployed: string | null;
}

export interface StackListView {
    items: StackView[];
}

export interface StackPlanView {
    services: string[];
    networks: string[];
    /** Named volumes the services mount; removal leaves them. */
    volumesRetained: string[];
}

/** The `stack.remove` phases, as the dialog names them. */
export const STACK_REMOVAL_PHASES: Record<string, string> = {
    removing_services: "Removing services",
    waiting_for_tasks: "Waiting for tasks to stop",
    removing_networks: "Removing networks",
    removed: "Removed",
};
