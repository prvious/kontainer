import { notify } from "@/components/kit/notify";
import { useContainerAction, type ContainerRow } from "@/hooks/use-containers";
import { withIdempotencyKey } from "@/hooks/use-operations";
import type { ContainerAction } from "@/types/containers";

/** Start one container action as an operation; a refused admission says so in a toast. */
export function useRunContainerAction() {
    const action = useContainerAction();
    return function run(row: ContainerRow, containerAction: ContainerAction, options: { force?: boolean } | null = null) {
        action.mutate(withIdempotencyKey({ nodeId: row.nodeId, containerId: row.containerId, action: containerAction, options, name: row.name }), {
            onError: (code) =>
                notify.error(`Couldn't ${containerAction} ${row.name}`, { code: String(code), message: "The controller did not accept the action." }),
        });
    };
}
