import { ConfirmDialog } from "@/components/kit/confirm-dialog";
import { useRunContainerAction } from "@/components/containers/use-run-container-action";
import type { ContainerRow } from "@/hooks/use-containers";

/** Remove one container; a running one needs its name typed and is killed first. */
export function RemoveContainerDialog({ row, onClose }: { row: ContainerRow | null; onClose: () => void }) {
    const run = useRunContainerAction();
    const running = row?.state === "running";
    return (
        <ConfirmDialog
            open={row !== null}
            onOpenChange={(open) => !open && onClose()}
            title={`Remove ${row?.name ?? "container"}?`}
            description={
                running
                    ? "It is running. Removing kills it first and deletes its writable layer. This cannot be undone."
                    : "Its writable layer is deleted. This cannot be undone."
            }
            confirmationText={running ? row.name : undefined}
            confirmLabel='REMOVE'
            onConfirm={() => {
                if (row) run(row, "remove", running ? { force: true } : null);
                onClose();
            }}
        />
    );
}
