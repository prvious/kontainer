import { ConfirmDialog } from "@/components/kit/confirm-dialog";
import { needsForce } from "@/components/containers/container-format";
import { useRunContainerAction } from "@/components/containers/use-run-container-action";
import type { ContainerRow } from "@/hooks/use-containers";

/**
 * Remove one container. Its writable layer and logs are lost in every state, so the name is
 * always typed (ADR 0036); a running, paused, or restarting one is also killed first (`force`).
 */
export function RemoveContainerDialog({ row, onClose }: { row: ContainerRow | null; onClose: () => void }) {
    const run = useRunContainerAction();
    const force = row !== null && needsForce(row);
    return (
        <ConfirmDialog
            open={row !== null}
            onOpenChange={(open) => !open && onClose()}
            title={`Remove ${row?.name ?? "container"}?`}
            description={
                force
                    ? `It is ${row.state}. Removing kills it first and deletes its writable layer. This cannot be undone.`
                    : "Its writable layer is deleted. This cannot be undone."
            }
            confirmationText={row?.name}
            confirmLabel='REMOVE'
            onConfirm={() => {
                if (row) run(row, "remove", force ? { force: true } : null);
                onClose();
            }}
        />
    );
}
