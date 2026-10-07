import { Waypoints } from "lucide-react";
import { Skeleton } from "@/components/ui/skeleton";

/**
 * Traefik banner `Ykc5k`, always in its skeleton variant from `PsbU8` (ADR 0010): the identity tile
 * and title keep the layout, and the figures stay placeholders until the integration exists.
 */
export function TraefikBanner() {
    return (
        <section aria-label='Traefik (not connected)' className='flex h-18 items-center justify-between gap-6 rounded-md border border-border bg-card px-4'>
            <div className='flex items-center gap-3'>
                <div className='flex size-7.5 items-center justify-center rounded-md bg-secondary'>
                    <Waypoints aria-hidden className='size-3.75 text-secondary-foreground' />
                </div>
                <div className='flex flex-col gap-1.5'>
                    <h2 className='font-mono text-xs font-bold text-foreground'>TRAEFIK</h2>
                    <Skeleton className='h-2 w-40 rounded-sm bg-secondary' />
                </div>
            </div>
            <div aria-hidden className='flex items-center gap-7'>
                <Skeleton className='h-5.5 w-16 rounded-sm bg-secondary' />
                <Skeleton className='h-5.5 w-12 rounded-sm bg-secondary' />
                <Skeleton className='h-5.5 w-12 rounded-sm bg-secondary' />
                <Skeleton className='h-7.5 w-37 rounded-sm bg-secondary' />
            </div>
        </section>
    );
}
