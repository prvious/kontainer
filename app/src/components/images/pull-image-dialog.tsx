import { useId, useState } from "react";
import { CircleCheck, CircleX, Layers, Loader2, Minimize2, RotateCw } from "lucide-react";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { ActionButton } from "@/components/kit/action-button";
import { FormField } from "@/components/kit/form-field";
import { fieldHelperId } from "@/components/kit/field-ids";
import { notify } from "@/components/kit/notify";
import { PRIVATE_REGISTRY_HINT, detail, nodeProgress, percent, type NodeProgress, type NodeState, type PullAttempt } from "@/components/images/pull-progress";
import { useCancelOperation } from "@/hooks/use-cancel-operation";
import { isSettled, useOperationViews } from "@/hooks/use-operation";
import { withIdempotencyKey } from "@/hooks/use-operations";
import { usePullImage } from "@/hooks/use-images";
import { referenceProblem, REFERENCE_PROBLEMS } from "@/types/images";
import type { NodeSummary } from "@/types/nodes";
import { cn } from "@/lib/utils";

const STATE_ICON = {
    queued: <Loader2 aria-hidden className='size-3.5 text-muted-foreground' />,
    pulling: <Loader2 aria-hidden className='size-3.5 animate-spin text-primary' />,
    done: <CircleCheck aria-hidden className='size-3.5 text-success' />,
    failed: <CircleX aria-hidden className='size-3.5 text-destructive' />,
    cancelled: <CircleX aria-hidden className='size-3.5 text-muted-foreground' />,
} satisfies Record<NodeState, unknown>;

/**
 * Pull image (03.S8): a reference validated on blur and node checkboxes defaulting to every online
 * node, then one progress row per node from the operation's detail. A failing node keeps the
 * registry's message verbatim with a per-node RETRY; RUN IN BACKGROUND hands the operation to a
 * loading toast.
 */
export function PullImageDialog({ open, onOpenChange, nodes }: { open: boolean; onOpenChange: (open: boolean) => void; nodes: NodeSummary[] }) {
    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className='w-135 max-w-[calc(100vw-2rem)] gap-0 rounded-lg bg-popover p-0 ring-border sm:max-w-135'>
                {/* Mounted only while open, so every opening starts a fresh pull. */}
                {open && <PullImageBody nodes={nodes} onClose={() => onOpenChange(false)} />}
            </DialogContent>
        </Dialog>
    );
}

function PullImageBody({ nodes, onClose }: { nodes: NodeSummary[]; onClose: () => void }) {
    const referenceId = useId();
    const pull = usePullImage();
    const cancel = useCancelOperation();
    const [reference, setReference] = useState("");
    const [referenceError, setReferenceError] = useState<string | null>(null);
    const [selected, setSelected] = useState<string[]>(() => nodes.filter((node) => node.agentStatus === "online").map((node) => node.id));
    const [attempts, setAttempts] = useState<PullAttempt[]>([]);
    const views = useOperationViews(attempts.map((attempt) => attempt.operationId));
    const rows = nodeProgress(attempts, views);
    const pulling = attempts.length > 0;
    const trimmed = reference.trim();

    function validate(): boolean {
        const problem = referenceProblem(reference);
        setReferenceError(problem ? REFERENCE_PROBLEMS[problem] : null);
        return problem === null;
    }

    function start(nodeIds: string[]) {
        pull.mutate(withIdempotencyKey({ reference: trimmed, nodeIds }), {
            onSuccess: (admitted) => setAttempts((current) => [...current, { operationId: admitted.operationId, nodeIds }]),
            onError: (code) => {
                if (String(code) === "invalid_request" && attempts.length === 0) setReferenceError("The controller rejected this reference or node list.");
                else notify.error(`Couldn't pull ${trimmed}`, { code: String(code), message: "The controller did not accept the pull." });
            },
        });
    }

    function runInBackground() {
        attempts.forEach((attempt, index) => {
            const view = views[index];
            if (!view || !isSettled(view.state))
                notify.loading(`Pulling ${trimmed}`, {
                    id: attempt.operationId,
                    description: `${attempt.nodeIds.length === 1 ? attempt.nodeIds[0] : `${attempt.nodeIds.length} nodes`} · the image appears on the next refresh.`,
                });
        });
        onClose();
    }

    const done = rows.filter((row) => row.state === "done").length;
    const failed = rows.filter((row) => row.state === "failed").length;
    const settled = rows.length > 0 && rows.every((row) => row.state === "done" || row.state === "failed" || row.state === "cancelled");
    // Cancelling only works before a pull starts (ADR 0016): after that, nodes finish on their own.
    const cancellable = attempts.filter((_, index) => views[index]?.state === "recoverable");

    return (
        <form
            onSubmit={(event) => {
                event.preventDefault();
                if (!pulling && validate() && selected.length > 0) start(selected);
            }}
        >
            <div className='flex flex-col gap-1.5 px-5 pt-5 pr-10'>
                <DialogTitle className='font-sans text-base font-semibold text-foreground'>Pull image</DialogTitle>
                <DialogDescription className='font-mono text-2xs leading-relaxed text-muted-foreground'>
                    Pulls on every target node in parallel. You can close this — progress continues in the background.
                </DialogDescription>
            </div>
            <div className='flex flex-col gap-4 px-5 pt-4'>
                <FormField
                    id={referenceId}
                    label='Image reference'
                    error={referenceError}
                    helper={pulling ? undefined : "Public images only; :latest is added when no tag is given."}
                >
                    <label
                        className={cn(
                            "flex h-9 items-center gap-2 rounded-md border border-border bg-muted px-3 focus-within:border-ring",
                            referenceError && "border-destructive",
                        )}
                    >
                        <Layers aria-hidden className='size-3.5 shrink-0 text-muted-foreground' />
                        <Input
                            id={referenceId}
                            autoFocus
                            autoComplete='off'
                            spellCheck={false}
                            placeholder='alpine:3.20'
                            value={reference}
                            readOnly={pulling}
                            aria-invalid={referenceError ? true : undefined}
                            aria-describedby={fieldHelperId(referenceId)}
                            onChange={(event) => {
                                setReference(event.target.value);
                                if (referenceError) setReferenceError(null);
                            }}
                            onBlur={() => {
                                if (reference !== "") validate();
                            }}
                            className='h-auto border-0 bg-transparent p-0 font-mono text-xs shadow-none focus-visible:ring-0 dark:bg-transparent'
                        />
                        {pulling && <CircleCheck aria-hidden className='size-3.5 shrink-0 text-success' />}
                    </label>
                </FormField>
                <div className='flex flex-col gap-2'>
                    <span className='font-mono text-3xs font-medium tracking-[1px] text-muted-foreground uppercase'>
                        Target · {pulling ? `${rows.length} ${rows.length === 1 ? "node" : "nodes"}` : `${selected.length} of ${nodes.length} nodes`}
                    </span>
                    <div className='flex flex-col divide-y divide-border rounded-md border border-border'>
                        {pulling
                            ? rows.map((row) => (
                                  <NodeProgressRow
                                      key={row.nodeId}
                                      row={row}
                                      retrying={pull.isPending && pull.variables?.nodeIds[0] === row.nodeId}
                                      onRetry={() => start([row.nodeId])}
                                  />
                              ))
                            : nodes.map((node) => (
                                  <label key={node.id} className='flex h-10 items-center gap-3 px-3 font-mono text-2xs text-foreground'>
                                      <Checkbox
                                          aria-label={`Pull on ${node.id}`}
                                          checked={selected.includes(node.id)}
                                          disabled={node.agentStatus === "offline"}
                                          onCheckedChange={(checked) =>
                                              setSelected((current) => (checked === true ? [...current, node.id] : current.filter((id) => id !== node.id)))
                                          }
                                          className='size-3.5 rounded-sm'
                                      />
                                      <span className='flex-1 truncate'>{node.id}</span>
                                      <span className={cn("text-3xs uppercase", node.agentStatus === "online" ? "text-success" : "text-muted-foreground")}>
                                          {node.agentStatus}
                                      </span>
                                  </label>
                              ))}
                    </div>
                </div>
            </div>
            <div className='mt-5 flex items-center justify-between gap-3 border-t border-border px-5 py-3.5'>
                <span className='font-mono text-2xs text-muted-foreground'>
                    {pulling ? `${done} of ${rows.length} nodes done${failed > 0 ? ` · ${failed} failed` : ""}` : ""}
                </span>
                <div className='flex items-center gap-2'>
                    {pulling ? (
                        <>
                            {!settled && (
                                <ActionButton
                                    type='button'
                                    variant='secondary'
                                    className='h-7.75 px-3.5 text-2xs'
                                    pending={cancel.isPending}
                                    restricted={
                                        cancellable.length === 0
                                            ? { title: "Pulls can't be stopped once started", reason: "Nodes already downloading finish on their own." }
                                            : undefined
                                    }
                                    onClick={() => cancellable.forEach((attempt) => cancel.mutate(attempt.operationId))}
                                >
                                    CANCEL PULL
                                </ActionButton>
                            )}
                            {settled ? (
                                <ActionButton type='button' variant='primary' className='h-7.75 px-3.5 text-2xs' onClick={onClose}>
                                    DONE
                                </ActionButton>
                            ) : (
                                <ActionButton type='button' variant='primary' className='h-7.75 px-3.5 text-2xs' icon={Minimize2} onClick={runInBackground}>
                                    RUN IN BACKGROUND
                                </ActionButton>
                            )}
                        </>
                    ) : (
                        <>
                            <ActionButton type='button' variant='secondary' className='h-7.75 px-3.5 text-2xs' onClick={onClose}>
                                CANCEL
                            </ActionButton>
                            <ActionButton
                                type='submit'
                                variant='primary'
                                className='h-7.75 px-3.5 text-2xs'
                                pending={pull.isPending}
                                disabled={selected.length === 0 || trimmed === ""}
                                mutates
                            >
                                PULL
                            </ActionButton>
                        </>
                    )}
                </div>
            </div>
        </form>
    );
}

function NodeProgressRow({ row, retrying, onRetry }: { row: NodeProgress; retrying: boolean; onRetry: () => void }) {
    const value = percent(row.pull, row.state);
    return (
        <div data-testid={`pull-node-${row.nodeId}`} data-state={row.state} className='flex flex-col gap-1.5 px-3 py-2.5'>
            <div className='flex items-center gap-3'>
                {STATE_ICON[row.state]}
                <span className='w-32 truncate font-mono text-2xs text-foreground'>{row.nodeId}</span>
                <Progress
                    value={value}
                    aria-label={`${row.nodeId} pull progress`}
                    className={cn(
                        "w-30 flex-none gap-0",
                        row.state === "done" && "[&_[data-slot=progress-indicator]]:bg-success",
                        row.state === "failed" && "[&_[data-slot=progress-indicator]]:bg-destructive",
                    )}
                />
                <span
                    className={cn(
                        "ml-auto font-mono text-3xs",
                        row.state === "done" ? "text-success" : row.state === "failed" ? "text-destructive" : "text-secondary-foreground",
                    )}
                >
                    {detail(row)}
                </span>
            </div>
            {row.state === "failed" && (
                <div className='flex items-start gap-3 pl-6.5'>
                    <p className='flex-1 font-mono text-3xs leading-normal text-destructive'>
                        <span className='break-words'>{row.error}</span>
                        <span className='block text-muted-foreground'>{PRIVATE_REGISTRY_HINT}</span>
                    </p>
                    <ActionButton type='button' variant='outline' size='sm' icon={RotateCw} pending={retrying} mutates onClick={onRetry}>
                        RETRY
                    </ActionButton>
                </div>
            )}
        </div>
    );
}
