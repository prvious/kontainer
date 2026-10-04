import { ConfirmDialog } from "@/components/kit/confirm-dialog";
import { needsForce } from "@/components/containers/container-format";
import { useRunContainerAction } from "@/components/containers/use-run-container-action";
import type { ContainerRow } from "@/hooks/use-containers";

/** Remove one container; a running, paused, or restarting one needs its name typed and is killed first. */
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
            confirmationText={force ? row.name : undefined}
            confirmLabel='REMOVE'
            onConfirm={() => {
                if (row) run(row, "remove", force ? { force: true } : null);
                onClose();
            }}
        />
    );
}
