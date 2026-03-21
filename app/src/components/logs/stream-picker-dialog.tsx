import { useMemo, useState } from 'react';
import { Search } from 'lucide-react';
import { useServices } from '@/hooks/use-services';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Skeleton } from '@/components/ui/skeleton';
import type { ServiceItem } from '@/types/services';

interface StreamPickerDialogProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    onSelect: (service: { id: string; name: string }) => void;
    excludeServiceIds?: string[];
}

function ServicePickerRow({
    service,
    excluded,
    onSelect,
}: {
    service: ServiceItem;
    excluded: boolean;
    onSelect: () => void;
}) {
    const isGlobal = service.replicasDesired === -1;
    const replicasLabel = isGlobal ? 'global' : `${service.replicasRunning}/${service.replicasDesired}`;

    return (
        <button
            onClick={excluded ? undefined : onSelect}
            disabled={excluded}
            className={`flex w-full items-center gap-3 rounded-md px-3 py-2.5 text-left transition-colors ${
                excluded
                    ? 'cursor-not-allowed opacity-40'
                    : 'hover:bg-accent'
            }`}
        >
            <span className='mt-0.5 size-2 shrink-0 rounded-full bg-accent-green' />
            <div className='min-w-0 flex-1'>
                <p className='truncate font-mono text-sm font-semibold'>{service.name}</p>
                <p className='truncate font-mono text-xs text-muted-foreground'>{service.image}</p>
            </div>
            <div className='flex shrink-0 flex-col items-end gap-0.5'>
                {excluded ? (
                    <span className='font-mono text-[10px] text-muted-foreground'>Already streaming</span>
                ) : (
                    <span className='font-mono text-[10px] text-muted-foreground'>{replicasLabel} replicas</span>
                )}
            </div>
        </button>
    );
}

function SkeletonRows() {
    return (
        <div className='space-y-1 p-1'>
            {[1, 2, 3, 4].map((i) => (
                <div key={i} className='flex items-center gap-3 rounded-md px-3 py-2.5'>
                    <Skeleton className='size-2 rounded-full' />
                    <div className='flex-1 space-y-1.5'>
                        <Skeleton className='h-3.5 w-36' />
                        <Skeleton className='h-3 w-52' />
                    </div>
                    <Skeleton className='h-3 w-14' />
                </div>
            ))}
        </div>
    );
}

export function StreamPickerDialog({ open, onOpenChange, onSelect, excludeServiceIds = [] }: StreamPickerDialogProps) {
    const [search, setSearch] = useState('');

    const { data: services, isLoading } = useServices();

    const filtered = useMemo(() => {
        if (!services) return [];
        const query = search.trim().toLowerCase();
        if (!query) return services;
        return services.filter(
            (svc) => svc.name.toLowerCase().includes(query) || svc.image.toLowerCase().includes(query),
        );
    }, [services, search]);

    const excludedSet = useMemo(() => new Set(excludeServiceIds), [excludeServiceIds]);

    function handleSelect(service: ServiceItem) {
        onSelect({ id: service.id, name: service.name });
        onOpenChange(false);
    }

    const hasServices = services && services.length > 0;
    const hasResults = filtered.length > 0;

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent showCloseButton={false} className='gap-0 p-0 sm:max-w-md'>
                <DialogHeader className='border-b border-border px-4 pt-4 pb-3'>
                    <DialogTitle>Select Service</DialogTitle>
                    <DialogDescription>Choose a service to stream logs from</DialogDescription>
                </DialogHeader>

                {/* Search input */}
                <div className='flex items-center gap-2 border-b border-border px-3'>
                    <Search className='size-4 shrink-0 text-muted-foreground' />
                    <input
                        value={search}
                        onChange={(e) => setSearch(e.target.value)}
                        placeholder='Search by name or image...'
                        className='flex-1 bg-transparent py-3 text-sm outline-none placeholder:text-muted-foreground'
                        autoFocus
                    />
                </div>

                {/* Service list */}
                <ScrollArea className='max-h-80'>
                    {isLoading ? (
                        <SkeletonRows />
                    ) : !hasServices ? (
                        <div className='px-4 py-8 text-center'>
                            <p className='text-sm text-muted-foreground'>No services found</p>
                        </div>
                    ) : !hasResults ? (
                        <div className='px-4 py-8 text-center'>
                            <p className='text-sm text-muted-foreground'>No services match</p>
                        </div>
                    ) : (
                        <div className='p-1'>
                            {filtered.map((svc) => (
                                <ServicePickerRow
                                    key={svc.id}
                                    service={svc}
                                    excluded={excludedSet.has(svc.id)}
                                    onSelect={() => handleSelect(svc)}
                                />
                            ))}
                        </div>
                    )}
                </ScrollArea>
            </DialogContent>
        </Dialog>
    );
}
