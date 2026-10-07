import { useState } from "react";
import { Link } from "@tanstack/react-router";
import { HardDrive, Trash2 } from "lucide-react";
import { ConfirmDialog } from "@/components/kit/confirm-dialog";
import { notify } from "@/components/kit/notify";
import { isSettled, useOperationViews } from "@/hooks/use-operation";
import { withIdempotencyKey } from "@/hooks/use-operations";
import { useRemoveStack, useStackPlan } from "@/hooks/use-stacks";
import { STACK_REMOVAL_PHASES } from "@/types/stacks";

function Names({ label, names }: { label: string; names: string[] }) {
    return (
        <div className='flex gap-2'>
            <span className='shrink-0 font-semibold text-foreground'>
                {names.length} {label}
                {names.length === 1 ? "" : "s"}
            </span>
            <span className='min-w-0 text-muted-foreground'>{names.length > 0 ? names.join(" · ") : "none"}</span>
        </div>
    );
}

const UNCERTAIN = "Outcome uncertain — check the stack before retrying.";

/**
 * Remove stack (04.S2): the plan loads first — services and stack networks removed, named volumes
 * retained — then typing the stack name enables REMOVE STACK. While the operation runs the dialog
 * shows its phase; it closes on success. A failure stays in the dialog whatever the plan shows
 * afterwards: once the services are gone the plan itself may no longer load.
 */
export function RemoveStackDialog({ name, onClose }: { name: string | null; onClose: () => void }) {
    const plan = useStackPlan(name);
    // The last removal started here. It is kept after it settles: from then on something may have changed.
    const [operationId, setOperationId] = useState<string | null>(null);
    const [view] = useOperationViews(operationId ? [operationId] : []);

    function close() {
        setOperationId(null);
        onClose();
    }

    const remove = useRemoveStack((settled) => {
        if (settled.state === "succeeded") close();
    });

    const settled = view && isSettled(view.state) ? view : undefined;
    const running = remove.isPending || (operationId !== null && settled === undefined);
    const failure =
        running || !settled || settled.state === "succeeded"
            ? null
            : settled.state === "uncertain"
              ? UNCERTAIN
              : (settled.errorMessage ?? "The stack was not removed.");
    const phase = !settled && typeof view?.progress?.phase === "string" ? (STACK_REMOVAL_PHASES[view.progress.phase] ?? view.progress.phase) : null;
    const planContent = plan.isPending ? (
        <p className='text-muted-foreground'>Loading what will be removed…</p>
    ) : plan.isError ? (
        <p className='text-destructive'>
            The removal plan couldn't be loaded ({String(plan.error)}).{operationId === null && " Nothing was changed."}
        </p>
    ) : plan.data ? (
        <>
            <div className='flex flex-col gap-1.5 rounded-sm border border-destructive/40 bg-destructive-soft px-3 py-2.5'>
                <span className='text-3xs font-bold tracking-[1px] text-destructive uppercase'>Will be removed</span>
                <Names label='service' names={plan.data.services} />
                <Names label='network' names={plan.data.networks} />
            </div>
            <div className='flex flex-col gap-1.5 rounded-sm border border-success/40 bg-success-soft px-3 py-2.5'>
                <span className='text-3xs font-bold tracking-[1px] text-success uppercase'>Retained</span>
                <div className='flex items-start gap-2'>
                    <HardDrive aria-hidden className='mt-0.5 size-3 shrink-0 text-success' />
                    <Names label='volume' names={plan.data.volumesRetained} />
                </div>
            </div>
        </>
    ) : null;
    const content = (
        <div className='flex flex-col gap-2.5 font-mono text-2xs'>
            {planContent}
            {running && <p className='text-muted-foreground'>{phase ?? "Starting…"}</p>}
            {failure && (
                <p role='alert' className='text-destructive'>
                    {failure}{" "}
                    {settled?.state === "failed" && (
                        // Networks left behind by a half-finished removal are removed from there (D8).
                        <Link to='/networks' className='font-bold whitespace-nowrap text-sidebar-accent-foreground hover:underline'>
                            OPEN NETWORKS
                        </Link>
                    )}
                </p>
            )}
        </div>
    );

    return (
        <ConfirmDialog
            open={name !== null}
            onOpenChange={(open) => !open && !running && close()}
            title={`Remove stack ${name ?? ""}?`}
            description="Stops and removes every service in the stack and the networks it created. Running tasks are drained first; this can't be undone. Named volumes are kept."
            confirmationText={name ?? undefined}
            confirmLabel='REMOVE STACK'
            confirmIcon={Trash2}
            pending={running}
            onConfirm={() => {
                if (!name || !plan.data) return;
                remove.mutate(withIdempotencyKey({ name }), {
                    onSuccess: (admitted) => setOperationId(admitted.operationId),
                    onError: (code) =>
                        notify.error(`Couldn't remove stack ${name}`, {
                            code: String(code),
                            message:
                                String(code) === "quorum_unavailable"
                                    ? "The swarm has no quorum, so nothing was changed."
                                    : "The controller did not accept the removal.",
                        }),
                });
            }}
        >
            {content}
        </ConfirmDialog>
    );
}
