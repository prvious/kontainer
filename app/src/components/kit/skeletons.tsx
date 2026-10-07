import { cn } from "@/lib/utils";

function Bar({ className }: { className: string }) {
    return <div aria-hidden className={cn("rounded-[3px] bg-secondary", className)} />;
}

/** Skeleton Stat `tLPEH`: first-load stand-in for a metric card, or a permanent deferred panel (ADR 0010). */
export function SkeletonStat({ className }: { className?: string }) {
    return (
        <div data-testid='skeleton-stat' className={cn("flex flex-col gap-3.5 rounded-md border border-border bg-card p-4", className)}>
            <Bar className='h-2 w-16' />
            <Bar className='h-6 w-24' />
            <Bar className='h-2 w-38' />
        </div>
    );
}

/** Skeleton Row `ZZAe7`: one table row stand-in; `fade` drops opacity toward the bottom of the list. */
export function SkeletonRow({ fade = 1 }: { fade?: number }) {
    return (
        <div data-testid='skeleton-row' style={{ opacity: fade }} className='flex h-11 items-center gap-7 border-b border-border px-4'>
            <Bar className='size-3.5' />
            <Bar className='h-2 w-11' />
            <Bar className='h-2 w-38' />
            <Bar className='h-2 w-31' />
            <Bar className='h-2 w-9' />
            <Bar className='h-2 w-14' />
            <Bar className='h-2 w-11' />
        </div>
    );
}

export function SkeletonRows({ count = 6 }: { count?: number }) {
    return (
        <div aria-label='Loading' aria-busy>
            {Array.from({ length: count }, (_, index) => (
                <SkeletonRow key={index} fade={1 - index / (count + 1)} />
            ))}
        </div>
    );
}

/** A deferred panel's body: static skeleton lines with no copy and no shimmer (ADR 0010). */
export function SkeletonLines({ lines = 3, className }: { lines?: number; className?: string }) {
    return (
        <div aria-hidden className={cn("flex flex-col gap-2", className)}>
            {Array.from({ length: lines }, (_, index) => (
                <Bar key={index} className={cn("h-2", index === lines - 1 ? "w-1/2" : "w-full")} />
            ))}
        </div>
    );
}
