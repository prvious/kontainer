import { OperationStatus } from "@/components/kit/operation-status";
import { notify } from "@/components/kit/notify";
import { operationMeta, operationPhase, operationTrack, pollErrorMeta } from "@/components/operations/operation-meta";
import { useCancelOperation } from "@/hooks/use-cancel-operation";
import { isPermanentPollError, useOperation } from "@/hooks/use-operation";
import { useOperations } from "@/hooks/use-operations";
import { useNow } from "@/hooks/use-now";
import type { TrackedOperation } from "@/contexts/operations-context-value";

/**
 * One pinned operation: CANCEL while it has not started, DISMISS once it failed or settled. A poll
 * that failed with nothing to show, or failed for good, reads as unknown with DISMISS.
 */
export function OperationRow({ operation }: { operation: TrackedOperation }) {
    const snapshot = useOperation(operation.id);
    const { dismiss } = useOperations();
    const cancel = useCancelOperation();
    const now = useNow();
    const view = snapshot.data;
    const pollError = snapshot.isError && (!view || isPermanentPollError(snapshot.error)) ? String(snapshot.error) : null;
    const phase = pollError ? "uncertain" : operationPhase(view);

    async function cancelIt() {
        try {
            await cancel.mutateAsync(operation.id);
        } catch (code) {
            notify.error(`Couldn't cancel ${operation.title.toLowerCase()}`, {
                code: String(code),
                message: code === "conflict" ? "It already started; running operations finish." : "The controller did not accept the cancel.",
            });
        }
    }

    const action =
        phase === "queued"
            ? { label: cancel.isPending ? "CANCELLING…" : "CANCEL", onClick: () => void cancelIt() }
            : phase === "running"
              ? undefined
              : { label: "DISMISS", onClick: () => dismiss(operation.id) };
    return (
        <OperationStatus
            phase={phase}
            title={operation.title}
            meta={pollError ? pollErrorMeta(pollError) : operationMeta(view, now)}
            progress={operationTrack(view)}
            action={action}
            className={phase === "failed" ? "border-destructive-soft" : undefined}
        />
    );
}
