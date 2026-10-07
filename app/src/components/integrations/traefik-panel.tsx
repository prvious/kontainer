import { Route as RouteIcon } from "lucide-react";
import { StatusBadge } from "@/components/kit/badges";
import { SkeletonLines } from "@/components/kit/skeletons";

/**
 * The hub's Traefik panel `xu4By`, deferred (ADR 0010): identity tile, title, and NOT
 * CONFIGURED, with a skeleton block where the request chart and its footer sit.
 */
export function TraefikPanel() {
    return (
        <section aria-label='Traefik (not configured)' className='flex flex-col gap-5 rounded-md border border-border bg-card p-5'>
            <div className='flex items-center justify-between gap-3'>
                <div className='flex items-center gap-3'>
                    <div className='flex size-8.5 items-center justify-center rounded-md border border-border bg-secondary'>
                        <RouteIcon aria-hidden className='size-4 text-secondary-foreground' />
                    </div>
                    <div className='flex flex-col gap-1.5'>
                        <h2 className='font-mono text-sm font-bold tracking-[1px] text-foreground'>TRAEFIK</h2>
                        <SkeletonLines lines={1} className='w-72' />
                    </div>
                </div>
                <StatusBadge tone='neutral'>NOT CONFIGURED</StatusBadge>
            </div>
            <div aria-hidden className='h-64 rounded-md bg-secondary' />
            <SkeletonLines lines={1} className='w-80' />
        </section>
    );
}
