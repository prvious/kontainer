import { HardDrive } from "lucide-react";
import { StatusDot } from "@/components/kit/badges";
import { bytes } from "@/components/containers/container-format";
import { allocated } from "@/components/volumes/volume-search";
import type { VolumeView } from "@/types/volumes";

/**
 * Volume Storage strip `htkOl`: ATTACHED / UNATTACHED legend, ALLOCATED as a plain number (no
 * ring: there is no real ceiling), and GROWTH · 30d as a static skeleton because nothing records
 * volume sizes over time (ADR 0010).
 */
export function VolumeStorageStrip({ volumes }: { volumes: VolumeView[] }) {
    const attached = volumes.filter((volume) => volume.usedBy > 0).length;
    const total = allocated(volumes);
    const [value, unit] = bytes(total.bytes).split(" ");

    return (
        <section aria-label='Volume storage' className='flex h-18 items-center justify-between gap-6 rounded-md bg-card px-4'>
            <div className='flex items-center gap-3'>
                <div className='flex size-7.5 items-center justify-center rounded-md bg-secondary'>
                    <HardDrive aria-hidden className='size-3.75 text-secondary-foreground' />
                </div>
                <div className='flex flex-col gap-1.25'>
                    <h2 className='font-mono text-xs font-bold text-foreground'>VOLUME STORAGE</h2>
                    <div className='flex items-center gap-3 font-mono text-3xs font-semibold text-secondary-foreground'>
                        <span className='flex items-center gap-1.25'>
                            <StatusDot tone='success' /> {attached} ATTACHED
                        </span>
                        <span className='flex items-center gap-1.25'>
                            <StatusDot tone='warning' /> {volumes.length - attached} UNATTACHED
                        </span>
                    </div>
                </div>
            </div>
            <div className='flex items-center gap-5.5'>
                <div className='flex flex-col gap-1.25'>
                    <span className='font-mono text-3xs font-bold text-muted-foreground'>ALLOCATED</span>
                    <span className='flex items-baseline gap-1.25'>
                        <span className='font-sans text-lg font-bold text-foreground'>{value}</span>
                        <span className='font-mono text-3xs font-medium text-secondary-foreground'>
                            {unit}
                            {total.pending ? " · sizes pending" : ""}
                        </span>
                    </span>
                </div>
                <div aria-hidden className='h-6.5 w-px bg-border' />
                <div className='flex flex-col gap-1.5'>
                    <div data-testid='growth-trend-skeleton' aria-hidden className='flex h-7.5 w-29.5 items-end gap-0.75'>
                        {[30, 35, 38, 42, 45, 50, 52, 58, 62, 66, 72, 80].map((height, index) => (
                            <span key={index} className='flex-1 rounded-[1px] bg-secondary' style={{ height: `${height}%` }} />
                        ))}
                    </div>
                    <span className='font-mono text-3xs font-bold text-muted-foreground'>GROWTH · 30d</span>
                </div>
            </div>
        </section>
    );
}
