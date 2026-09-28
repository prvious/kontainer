import { useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { ArrowRight, Ban, Trash2 } from "lucide-react";
import { Popover, PopoverContent, PopoverDescription, PopoverTitle, PopoverTrigger } from "@/components/ui/popover";
import { ActionButton } from "@/components/kit/action-button";
import { LabelBadge } from "@/components/kit/badges";
import { useNetworkContainers } from "@/hooks/use-networks";
import { networkNode, type NetworkView } from "@/types/networks";

const SHOWN = 4;

/**
 * Remove blocked (03.S10): the trash of a network with endpoints opens this instead of a confirm.
 * The attached names are fetched when it opens; VIEW CONTAINERS filters the Containers page by
 * this network.
 */
export function RemoveBlockedPopover({ network }: { network: NetworkView }) {
    const navigate = useNavigate();
    const [open, setOpen] = useState(false);
    const attached = useNetworkContainers(open ? { id: network.id, nodeId: networkNode(network) } : null);
    const names = attached.data?.map((container) => container.name) ?? [];
    const count = attached.data?.length ?? network.containers;

    return (
        <Popover open={open} onOpenChange={setOpen}>
            <PopoverTrigger
                aria-label='Remove'
                className='flex size-5 items-center justify-center rounded-sm text-muted-foreground hover:bg-secondary hover:text-foreground'
            >
                <Trash2 aria-hidden className='size-3.5' />
            </PopoverTrigger>
            <PopoverContent align='end' className='w-95 gap-0 border-border bg-popover p-0'>
                <div className='flex gap-3.5 p-4'>
                    <div className='flex size-8 shrink-0 items-center justify-center rounded-md bg-warning-soft text-warning'>
                        <Ban aria-hidden className='size-4' />
                    </div>
                    <div className='flex min-w-0 flex-col gap-2'>
                        <PopoverTitle className='font-sans text-sm font-semibold text-foreground'>Can't remove {network.name}</PopoverTitle>
                        <PopoverDescription className='font-mono text-2xs leading-relaxed text-muted-foreground'>
                            {count} {count === 1 ? "container is" : "containers are"} still attached. Disconnect them, or remove the services that use this
                            network, then try again.
                        </PopoverDescription>
                        {attached.isPending ? (
                            <span className='font-mono text-3xs text-muted-foreground'>Loading attached containers…</span>
                        ) : attached.isError ? (
                            <span className='font-mono text-3xs text-destructive'>The attached containers couldn't be listed.</span>
                        ) : (
                            <div className='flex flex-wrap items-center gap-1.5'>
                                {names.slice(0, SHOWN).map((name) => (
                                    <LabelBadge key={name}>{name}</LabelBadge>
                                ))}
                                {names.length > SHOWN && <span className='font-mono text-3xs text-muted-foreground'>+{names.length - SHOWN}</span>}
                            </div>
                        )}
                    </div>
                </div>
                <div className='flex items-center justify-end gap-2 border-t border-border px-4 py-3'>
                    <ActionButton type='button' variant='secondary' size='sm' onClick={() => setOpen(false)}>
                        CLOSE
                    </ActionButton>
                    <ActionButton
                        variant='primary'
                        size='sm'
                        type='button'
                        onClick={() => void navigate({ to: "/containers", search: { network: network.name } })}
                    >
                        VIEW CONTAINERS
                        <ArrowRight aria-hidden />
                    </ActionButton>
                </div>
            </PopoverContent>
        </Popover>
    );
}
