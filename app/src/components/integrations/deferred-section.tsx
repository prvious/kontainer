import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";

function Bar({ className }: { className: string }) {
    return <div className={cn("h-2 rounded-[3px] bg-secondary", className)} />;
}

/**
 * A section of the GitHub page (`kx4iP`) in its deferred state (G23, ADR 0010): the designed
 * header, then static skeleton rows where its settings would be. No controls.
 */
export function DeferredSection({ icon: Icon, title, rows, danger = false }: { icon: LucideIcon; title: string; rows: number; danger?: boolean }) {
    return (
        <section aria-label={title} className={cn("overflow-hidden rounded-md border bg-card", danger ? "border-destructive" : "border-border")}>
            <div className='flex h-11 items-center gap-2.5 px-4'>
                <Icon aria-hidden className={cn("size-3.5", danger ? "text-destructive" : "text-muted-foreground")} />
                <h2 className={cn("font-mono text-xs font-bold tracking-[1px] uppercase", danger ? "text-destructive" : "text-foreground")}>{title}</h2>
            </div>
            <div aria-hidden className='divide-y divide-border border-t border-border'>
                {Array.from({ length: rows }, (_, index) => (
                    <div key={index} data-testid='skeleton-row' className='flex h-12.5 items-center justify-between gap-6 px-4'>
                        <div className='flex flex-col gap-2'>
                            <Bar className='w-28' />
                            <Bar className='w-52' />
                        </div>
                        <Bar className='w-20' />
                    </div>
                ))}
            </div>
        </section>
    );
}
