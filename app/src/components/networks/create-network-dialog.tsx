import { useId, useState } from "react";
import { Network, Plus, Server } from "lucide-react";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { ActionButton } from "@/components/kit/action-button";
import { FormField } from "@/components/kit/form-field";
import { fieldHelperId } from "@/components/kit/field-ids";
import { notify } from "@/components/kit/notify";
import { withIdempotencyKey } from "@/hooks/use-operations";
import { useCreateNetwork } from "@/hooks/use-networks";
import { networkNameProblem } from "@/components/networks/network-search";
import { isCidr, networkRowId, type NetworkView } from "@/types/networks";
import type { NodeSummary } from "@/types/nodes";
import { cn } from "@/lib/utils";

type Driver = "overlay" | "bridge";

/**
 * Create network (G1, modelled on 03.S9): NAME checked on blur against the pattern and the
 * current list, DRIVER overlay (default) or bridge, NODE only for bridge, ATTACHABLE only for
 * overlay, optional SUBNET as a CIDR. CREATE NETWORK waits for the operation; on success the
 * dialog closes and `onCreated` highlights the new row.
 */
export function CreateNetworkDialog({
    open,
    onOpenChange,
    nodes,
    networks,
    onCreated,
}: {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    nodes: NodeSummary[];
    networks: NetworkView[];
    onCreated: (rowId: string) => void;
}) {
    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className='w-125 max-w-[calc(100vw-2rem)] gap-0 rounded-lg bg-popover p-0 ring-border sm:max-w-125'>
                {open && <CreateNetworkBody nodes={nodes} networks={networks} onClose={() => onOpenChange(false)} onCreated={onCreated} />}
            </DialogContent>
        </Dialog>
    );
}

function CreateNetworkBody({
    nodes,
    networks,
    onClose,
    onCreated,
}: {
    nodes: NodeSummary[];
    networks: NetworkView[];
    onClose: () => void;
    onCreated: (rowId: string) => void;
}) {
    const nameId = useId();
    const nodeId = useId();
    const subnetId = useId();
    const attachableId = useId();
    const online = nodes.filter((node) => node.agentStatus === "online");
    const [name, setName] = useState("");
    const [nameChecked, setNameChecked] = useState(false);
    const [driver, setDriver] = useState<Driver>("overlay");
    const [node, setNode] = useState<string | null>(online[0]?.id ?? null);
    const [attachable, setAttachable] = useState(true);
    const [subnet, setSubnet] = useState("");
    const [subnetChecked, setSubnetChecked] = useState(false);
    const [settleError, setSettleError] = useState<string | null>(null);
    const [waiting, setWaiting] = useState(false);
    const create = useCreateNetwork((view, variables) => {
        if (view.state === "succeeded") {
            const id = typeof view.progress?.networkId === "string" ? view.progress.networkId : "";
            onCreated(networkRowId({ id, scope: variables.driver === "overlay" ? "swarm" : "local", nodeIds: variables.nodeId ? [variables.nodeId] : [] }));
        } else {
            setWaiting(false);
            setSettleError(view.errorMessage ?? "The network was not created.");
        }
    });

    const nameProblem = networkNameProblem(name, networks);
    const subnetProblem = subnet !== "" && !isCidr(subnet) ? "A network address with its prefix, like 10.0.9.0/24." : null;
    const invalid = nameProblem !== null || subnetProblem !== null || (driver === "bridge" && node === null);

    function submit() {
        setNameChecked(true);
        setSubnetChecked(true);
        if (invalid) return;
        setSettleError(null);
        setWaiting(true);
        create.mutate(
            withIdempotencyKey({
                name,
                driver,
                attachable: driver === "overlay" && attachable,
                nodeId: driver === "bridge" ? node : null,
                subnet: subnet || null,
            }),
            {
                onError: (code) => {
                    setWaiting(false);
                    notify.error(`Couldn't create ${name}`, {
                        code: String(code),
                        message:
                            String(code) === "not_a_manager"
                                ? "The controller's Docker host is not a Swarm manager."
                                : "The controller did not accept the network.",
                    });
                },
            },
        );
    }

    const shownNameError = (nameChecked && name !== "" ? nameProblem : null) ?? settleError;
    return (
        <form
            onSubmit={(event) => {
                event.preventDefault();
                submit();
            }}
        >
            <div className='flex flex-col gap-1.5 px-5 pt-5 pr-10'>
                <DialogTitle className='font-sans text-base font-semibold text-foreground'>Create network</DialogTitle>
                <DialogDescription className='font-mono text-2xs leading-relaxed text-muted-foreground'>
                    Overlay networks span the swarm and are created on the manager. Bridge networks live on one node.
                </DialogDescription>
            </div>
            <div className='flex flex-col gap-4 px-5 pt-4'>
                <FormField id={nameId} label='Name' error={shownNameError}>
                    <label
                        className={cn(
                            "flex h-9 items-center gap-2 rounded-md border border-border bg-muted px-3 focus-within:border-ring",
                            shownNameError && "border-destructive",
                        )}
                    >
                        <Network aria-hidden className='size-3.5 shrink-0 text-muted-foreground' />
                        <Input
                            id={nameId}
                            autoFocus
                            autoComplete='off'
                            spellCheck={false}
                            placeholder='app-network'
                            value={name}
                            aria-invalid={shownNameError ? true : undefined}
                            aria-describedby={fieldHelperId(nameId)}
                            onChange={(event) => {
                                setName(event.target.value);
                                setSettleError(null);
                            }}
                            onBlur={() => setNameChecked(true)}
                            className='h-auto border-0 bg-transparent p-0 font-mono text-xs shadow-none focus-visible:ring-0 dark:bg-transparent'
                        />
                    </label>
                </FormField>
                <fieldset className='flex flex-col gap-2'>
                    <legend className='mb-2 font-mono text-3xs font-medium tracking-[1px] text-muted-foreground uppercase'>Driver</legend>
                    <RadioGroup value={driver} onValueChange={(value) => setDriver(value === "bridge" ? "bridge" : "overlay")} className='flex gap-5'>
                        {(["overlay", "bridge"] as const).map((option) => (
                            <label key={option} className='flex items-center gap-2 font-mono text-2xs text-secondary-foreground'>
                                <RadioGroupItem value={option} aria-label={option} />
                                {option}
                                <span className='text-3xs text-muted-foreground'>{option === "overlay" ? "swarm-wide" : "one node"}</span>
                            </label>
                        ))}
                    </RadioGroup>
                </fieldset>
                {driver === "bridge" ? (
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
                ) : (
                    <div className='flex items-center justify-between gap-4'>
                        <div className='flex flex-col gap-0.5'>
                            <Label htmlFor={attachableId} className='font-mono text-3xs font-medium tracking-[1px] text-muted-foreground uppercase'>
                                Attachable
                            </Label>
                            <span className='font-mono text-3xs text-muted-foreground'>Standalone containers may join it, not only services.</span>
                        </div>
                        <Switch id={attachableId} checked={attachable} onCheckedChange={(checked) => setAttachable(checked)} />
                    </div>
                )}
                <FormField id={subnetId} label='Subnet · optional' error={subnetChecked ? subnetProblem : null} helper='Leave empty to let Docker pick one.'>
                    <Input
                        id={subnetId}
                        autoComplete='off'
                        spellCheck={false}
                        placeholder='10.0.9.0/24'
                        value={subnet}
                        aria-invalid={subnetChecked && subnetProblem ? true : undefined}
                        aria-describedby={fieldHelperId(subnetId)}
                        onChange={(event) => setSubnet(event.target.value.trim())}
                        onBlur={() => setSubnetChecked(true)}
                        className='h-9 rounded-md bg-muted font-mono text-xs'
                    />
                </FormField>
            </div>
            <div className='mt-5 flex items-center justify-end gap-2 border-t border-border px-5 py-3.5'>
                <ActionButton type='button' variant='secondary' className='h-7.75 px-3.5 text-2xs' onClick={onClose}>
                    CANCEL
                </ActionButton>
                <ActionButton
                    type='submit'
                    variant='primary'
                    className='h-7.75 px-3.5 text-2xs'
                    icon={Plus}
                    disabled={invalid}
                    pending={create.isPending || waiting}
                    mutates
                >
                    CREATE NETWORK
                </ActionButton>
            </div>
        </form>
    );
}
