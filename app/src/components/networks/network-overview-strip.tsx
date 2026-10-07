import { Network } from "lucide-react";
import { StatusDot } from "@/components/kit/badges";
import { Ring } from "@/components/charts/ring";
import type { NetworkView } from "@/types/networks";

/**
 * Network Overview strip `NRFey`: ACTIVE (with endpoints) / EMPTY legend, a Ring of active over
 * total, and the TRAFFIC · 60m trend as a static skeleton because nothing samples network
 * traffic per network (ADR 0010).
 */
export function NetworkOverviewStrip({ networks }: { networks: NetworkView[] }) {
    const active = networks.filter((network) => network.containers > 0).length;

    return (
        <section aria-label='Network overview' className='flex h-18 items-center justify-between gap-6 rounded-md bg-card px-4'>
            <div className='flex items-center gap-3'>
                <div className='flex size-7.5 items-center justify-center rounded-md bg-secondary'>
                    <Network aria-hidden className='size-3.75 text-secondary-foreground' />
                </div>
                <div className='flex flex-col gap-1.25'>
                    <h2 className='font-mono text-xs font-bold text-foreground'>NETWORK OVERVIEW</h2>
                    <div className='flex items-center gap-3 font-mono text-3xs font-semibold text-secondary-foreground'>
                        <span className='flex items-center gap-1.25'>
                            <StatusDot tone='success' /> {active} ACTIVE
                        </span>
                        <span className='flex items-center gap-1.25'>
                            <StatusDot tone='warning' /> {networks.length - active} EMPTY
                        </span>
                    </div>
                </div>
            </div>
            <div className='flex items-center gap-5.5'>
                <div className='flex items-center gap-2.5'>
                    <div className='flex flex-col gap-1.25'>
                        <span className='font-mono text-3xs font-bold text-muted-foreground'>ACTIVE</span>
                        <span className='flex items-baseline gap-1.25'>
                            <span className='font-sans text-lg font-bold text-foreground'>{active}</span>
                            <span className='font-mono text-3xs font-medium text-secondary-foreground'>/ {networks.length}</span>
                        </span>
                    </div>
                    <Ring
                        value={networks.length === 0 ? null : active / networks.length}
                        size={38}
                        thickness={4}
                        label={`${active} of ${networks.length} networks active`}
                    />
                </div>
                <div aria-hidden className='h-6.5 w-px bg-border' />
                <div className='flex flex-col gap-1.5'>
                    <div data-testid='traffic-trend-skeleton' aria-hidden className='flex h-7.5 w-29.5 items-end gap-0.75'>
                        {[45, 30, 60, 50, 70, 40, 65, 55, 80, 50, 60, 75].map((height, index) => (
                            <span key={index} className='flex-1 rounded-[1px] bg-secondary' style={{ height: `${height}%` }} />
                        ))}
                    </div>
                    <span className='font-mono text-3xs font-bold text-muted-foreground'>TRAFFIC · 60m</span>
                </div>
            </div>
        </section>
    );
}
