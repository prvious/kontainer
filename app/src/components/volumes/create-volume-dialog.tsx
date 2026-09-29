import { useId, useState } from "react";
import { Database, HardDrive, Plus, Server, X } from "lucide-react";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { ActionButton } from "@/components/kit/action-button";
import { FormField } from "@/components/kit/form-field";
import { fieldHelperId } from "@/components/kit/field-ids";
import { notify } from "@/components/kit/notify";
import { withIdempotencyKey } from "@/hooks/use-operations";
import { useCreateVolume, useVolumeNameTaken } from "@/hooks/use-volumes";
import { LABEL_PROBLEMS, parseLabel, VOLUME_NAME } from "@/types/volumes";
import type { NodeSummary } from "@/types/nodes";
import { cn } from "@/lib/utils";

const NAME_RULE = "2–255 letters, digits, and . _ -, starting with a letter or digit.";

/**
 * Create volume (03.S9): NODE among online nodes, NAME checked on blur against that node, DRIVER
 * fixed to `local`, optional labels. CREATE VOLUME waits for the operation; on success the dialog
 * closes and `onCreated` highlights the new row.
 */
export function CreateVolumeDialog({
    open,
    onOpenChange,
    nodes,
    onCreated,
}: {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    nodes: NodeSummary[];
    onCreated: (volume: { nodeId: string; name: string }) => void;
}) {
    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className='w-125 max-w-[calc(100vw-2rem)] gap-0 rounded-lg bg-popover p-0 ring-border sm:max-w-125'>
                {/* Mounted only while open, so every opening starts blank. */}
                {open && <CreateVolumeBody nodes={nodes} onClose={() => onOpenChange(false)} onCreated={onCreated} />}
            </DialogContent>
        </Dialog>
    );
}

function CreateVolumeBody({
    nodes,
    onClose,
    onCreated,
}: {
    nodes: NodeSummary[];
    onClose: () => void;
    onCreated: (volume: { nodeId: string; name: string }) => void;
}) {
    const nameId = useId();
    const nodeId = useId();
    const labelId = useId();
    const online = nodes.filter((node) => node.agentStatus === "online");
    const [node, setNode] = useState<string | null>(online[0]?.id ?? null);
    const [name, setName] = useState("");
    // The name last left (blur or submit): format and conflict checks run against it.
    const [checkedName, setCheckedName] = useState<string | null>(null);
    const [labels, setLabels] = useState<Record<string, string>>({});
    const [labelDraft, setLabelDraft] = useState<string | null>(null);
    const [labelError, setLabelError] = useState<string | null>(null);
    const [settleError, setSettleError] = useState<string | null>(null);
    const taken = useVolumeNameTaken(node, checkedName);
    const create = useCreateVolume((view, variables) => {
        if (view.state === "succeeded") onCreated({ nodeId: variables.nodeId, name: variables.name });
        else setSettleError(view.errorMessage ?? "The volume was not created.");
    });
    const [waiting, setWaiting] = useState(false);

    const formatInvalid = checkedName !== null && !VOLUME_NAME.test(checkedName);
    const conflict = checkedName === name && taken.data === true;
    const nameError = formatInvalid ? NAME_RULE : conflict ? `A volume named ${name} already exists on ${node}. Pick another name or another node.` : null;
    const invalid = node === null || !VOLUME_NAME.test(name) || conflict || (checkedName === name && taken.isFetching) || labelDraft !== null;

    function addLabel() {
        if (labelDraft === null || labelDraft.trim() === "") {
            setLabelDraft(null);
            setLabelError(null);
            return;
        }
        const parsed = parseLabel(labelDraft, labels);
        if (typeof parsed === "string") {
            setLabelError(LABEL_PROBLEMS[parsed]);
            return;
        }
        setLabels({ ...labels, [parsed.key]: parsed.value });
        setLabelDraft(null);
        setLabelError(null);
    }

    function submit() {
        setCheckedName(name);
        if (invalid || node === null) return;
        setSettleError(null);
        setWaiting(true);
        create.mutate(withIdempotencyKey({ nodeId: node, name, labels }), {
            onError: (code) => {
                setWaiting(false);
                notify.error(`Couldn't create ${name}`, { code: String(code), message: "The controller did not accept the volume." });
            },
        });
    }

    const pending = create.isPending || (waiting && settleError === null);
    return (
        <form
            onSubmit={(event) => {
                event.preventDefault();
                submit();
            }}
        >
            <div className='flex flex-col gap-1.5 px-5 pt-5 pr-10'>
                <DialogTitle className='font-sans text-base font-semibold text-foreground'>Create volume</DialogTitle>
                <DialogDescription className='font-mono text-2xs leading-relaxed text-muted-foreground'>
                    Local volumes live on a single node. Services that mount it will be pinned to that node.
                </DialogDescription>
            </div>
            <div className='flex flex-col gap-4 px-5 pt-4'>
                <FormField id={nameId} label='Name' error={nameError ?? settleError}>
                    <label
                        className={cn(
                            "flex h-9 items-center gap-2 rounded-md border border-border bg-muted px-3 focus-within:border-ring",
                            (nameError ?? settleError) && "border-destructive",
                        )}
                    >
                        <HardDrive aria-hidden className='size-3.5 shrink-0 text-muted-foreground' />
                        <Input
                            id={nameId}
                            autoFocus
                            autoComplete='off'
                            spellCheck={false}
                            placeholder='postgres-data'
                            value={name}
                            aria-invalid={nameError ? true : undefined}
                            aria-describedby={fieldHelperId(nameId)}
                            onChange={(event) => {
                                setName(event.target.value);
                                setSettleError(null);
                            }}
                            onBlur={() => {
                                if (name !== "") setCheckedName(name);
                            }}
                            className='h-auto border-0 bg-transparent p-0 font-mono text-xs shadow-none focus-visible:ring-0 dark:bg-transparent'
                        />
                    </label>
                </FormField>
                <div className='grid grid-cols-2 gap-3'>
                    <FormField id={nodeId} label='Node' helper={online.length === 0 ? "No node is online." : undefined}>
                        <Select
                            value={node}
                            onValueChange={(value) => setNode(typeof value === "string" ? value : null)}
                            items={online.map((entry) => ({ value: entry.id, label: entry.id }))}
                        >
                            <SelectTrigger id={nodeId} className='h-9 w-full rounded-md bg-muted font-mono text-xs'>
                                <Server aria-hidden className='size-3.5 text-muted-foreground' />
                                <SelectValue placeholder='Pick a node' />
                            </SelectTrigger>
                            <SelectContent>
                                {online.map((entry) => (
                                    <SelectItem key={entry.id} value={entry.id} className='font-mono text-xs'>
                                        {entry.id}
                                    </SelectItem>
                                ))}
                            </SelectContent>
                        </Select>
                    </FormField>
                    <FormField id={`${nodeId}-driver`} label='Driver'>
                        <div
                            id={`${nodeId}-driver`}
                            aria-disabled
                            className='flex h-9 items-center gap-2 rounded-md border border-border bg-muted px-3 font-mono text-xs text-secondary-foreground'
                        >
                            <Database aria-hidden className='size-3.5 text-muted-foreground' />
                            local
                        </div>
                    </FormField>
                </div>
                <FormField id={labelId} label='Labels · optional' error={labelError}>
                    <div className='flex flex-wrap items-center gap-2'>
                        {Object.entries(labels).map(([key, value]) => (
                            <span
                                key={key}
                                className='inline-flex items-center gap-1.5 rounded-sm border border-border bg-secondary px-2 py-1 font-mono text-3xs text-secondary-foreground'
                            >
                                {key}={value}
                                <button
                                    type='button'
                                    aria-label={`Remove label ${key}`}
                                    onClick={() => {
                                        const next = { ...labels };
                                        delete next[key];
                                        setLabels(next);
                                    }}
                                    className='text-muted-foreground hover:text-foreground'
                                >
                                    <X aria-hidden className='size-3' />
                                </button>
                            </span>
                        ))}
                        {labelDraft === null ? (
                            <button
                                type='button'
                                onClick={() => setLabelDraft("")}
                                className='inline-flex items-center gap-1 font-mono text-3xs font-bold tracking-[1px] text-secondary-foreground uppercase hover:text-foreground'
                            >
                                <Plus aria-hidden className='size-3' /> Add label
                            </button>
                        ) : (
                            <Input
                                id={labelId}
                                autoFocus
                                autoComplete='off'
                                spellCheck={false}
                                placeholder='key=value'
                                aria-label='New label'
                                value={labelDraft}
                                onChange={(event) => {
                                    setLabelDraft(event.target.value);
                                    setLabelError(null);
                                }}
                                onKeyDown={(event) => {
                                    if (event.key === "Enter") {
                                        event.preventDefault();
                                        addLabel();
                                    } else if (event.key === "Escape") {
                                        event.stopPropagation();
                                        setLabelDraft(null);
                                        setLabelError(null);
                                    }
                                }}
                                onBlur={addLabel}
                                className='h-7 w-44 rounded-sm bg-muted font-mono text-3xs'
                            />
                        )}
                    </div>
                </FormField>
            </div>
            <div className='mt-5 flex items-center justify-end gap-2 border-t border-border px-5 py-3.5'>
                <ActionButton type='button' variant='secondary' className='h-7.75 px-3.5 text-2xs' onClick={onClose}>
                    CANCEL
                </ActionButton>
                <ActionButton type='submit' variant='primary' className='h-7.75 px-3.5 text-2xs' icon={Plus} disabled={invalid} pending={pending} mutates>
                    CREATE VOLUME
                </ActionButton>
            </div>
        </form>
    );
}
