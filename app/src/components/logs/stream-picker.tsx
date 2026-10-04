import { Command, CommandDialog, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from "@/components/ui/command";
import { useServices } from "@/hooks/use-services";
import type { ServiceView } from "@/types/services";

/** G10 stream picker: every service, searchable; choosing one opens its tab. */
export function StreamPicker({ open, onOpenChange, onPick }: { open: boolean; onOpenChange: (open: boolean) => void; onPick: (service: ServiceView) => void }) {
    const services = useServices();
    const items = services.data?.items ?? [];
    return (
        <CommandDialog open={open} onOpenChange={onOpenChange} title='New log stream' description='Choose a service to follow.'>
            <Command>
                <CommandInput placeholder='Search services…' className='font-mono text-xs' />
                <CommandList>
                    <CommandEmpty className='py-6 text-center font-mono text-2xs text-muted-foreground'>
                        {services.isError ? "Services couldn't be loaded." : services.isPending ? "Loading services…" : "No service matches."}
                    </CommandEmpty>
                    <CommandGroup heading='SERVICES'>
                        {items.map((service) => (
                            <CommandItem
                                key={service.id}
                                value={`${service.name} ${service.id}`}
                                onSelect={() => {
                                    onPick(service);
                                    onOpenChange(false);
                                }}
                                className='font-mono text-xs'
                            >
                                <span className='flex-1 truncate'>{service.name}</span>
                                <span className='text-3xs text-muted-foreground'>{service.stack ?? "no stack"}</span>
                            </CommandItem>
                        ))}
                    </CommandGroup>
                </CommandList>
            </Command>
        </CommandDialog>
    );
}
