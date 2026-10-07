import { Command, CommandDialog, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from "@/components/ui/command";
import type { ExecTarget } from "@/contexts/exec-sessions-context-value";
import { useContainers } from "@/hooks/use-containers";

/** G11 target picker: running containers grouped by node; choosing one opens a shell. */
export function TargetPicker({ open, onOpenChange, onPick }: { open: boolean; onOpenChange: (open: boolean) => void; onPick: (target: ExecTarget) => void }) {
    const containers = useContainers();
    const running = (containers.data?.items ?? []).filter((row) => row.state === "running" && !row.stale);
    const nodes = [...new Set(running.map((row) => row.nodeId))].sort();
    return (
        <CommandDialog open={open} onOpenChange={onOpenChange} title='New shell' description='Choose a running container.'>
            <Command>
                <CommandInput placeholder='Search containers…' className='font-mono text-xs' />
                <CommandList>
                    <CommandEmpty className='py-6 text-center font-mono text-2xs text-muted-foreground'>
                        {containers.isError ? "Containers couldn't be loaded." : containers.isPending ? "Loading containers…" : "No running container matches."}
                    </CommandEmpty>
                    {nodes.map((node) => (
                        <CommandGroup key={node} heading={node.toUpperCase()}>
                            {running
                                .filter((row) => row.nodeId === node)
                                .map((row) => (
                                    <CommandItem
                                        key={`${row.nodeId}/${row.containerId}`}
                                        value={`${row.name} ${row.image} ${row.nodeId}`}
                                        onSelect={() => {
                                            onPick({ nodeId: row.nodeId, containerId: row.containerId, name: row.name });
                                            onOpenChange(false);
                                        }}
                                        className='font-mono text-xs'
                                    >
                                        <span className='flex-1 truncate'>{row.name}</span>
                                        <span className='truncate text-3xs text-muted-foreground'>{row.image}</span>
                                    </CommandItem>
                                ))}
                        </CommandGroup>
                    ))}
                </CommandList>
            </Command>
        </CommandDialog>
    );
}
