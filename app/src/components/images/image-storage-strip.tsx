import { Layers } from "lucide-react";
import { StatusDot } from "@/components/kit/badges";
import { bytes } from "@/components/containers/container-format";
import { diskUsageBytes } from "@/components/images/image-format";
import type { ImageView } from "@/types/images";

/**
 * Image Storage strip `InNiu`: IN USE / UNUSED legend, DISK USAGE as a plain number (no ring:
 * there is no real ceiling), and the PULLS · 14d trend as a static skeleton because nothing
 * records pulls over time (ADR 0010).
 */
export function ImageStorageStrip({ images }: { images: ImageView[] }) {
    const inUse = images.filter((image) => image.usedBy > 0).length;
    const [value, unit] = bytes(diskUsageBytes(images)).split(" ");
    const nodes = new Set(images.flatMap((image) => image.nodeIds)).size;

    return (
        <section aria-label='Image storage' className='flex h-18 items-center justify-between gap-6 rounded-md bg-card px-4'>
            <div className='flex items-center gap-3'>
                <div className='flex size-7.5 items-center justify-center rounded-md bg-secondary'>
                    <Layers aria-hidden className='size-3.75 text-secondary-foreground' />
                </div>
                <div className='flex flex-col gap-1.25'>
                    <h2 className='font-mono text-xs font-bold text-foreground'>IMAGE STORAGE</h2>
                    <div className='flex items-center gap-3 font-mono text-3xs font-semibold text-secondary-foreground'>
                        <span className='flex items-center gap-1.25'>
                            <StatusDot tone='success' /> {inUse} IN USE
                        </span>
                        <span className='flex items-center gap-1.25'>
                            <StatusDot tone='warning' /> {images.length - inUse} UNUSED
                        </span>
                    </div>
                </div>
            </div>
            <div className='flex items-center gap-5.5'>
                <div className='flex flex-col gap-1.25'>
                    <span className='font-mono text-3xs font-bold text-muted-foreground'>DISK USAGE</span>
                    <span className='flex items-baseline gap-1.25' aria-label={`${value} ${unit} on disk across ${nodes} ${nodes === 1 ? "node" : "nodes"}`}>
                        <span className='font-sans text-lg font-bold text-foreground'>{value}</span>
                        <span className='font-mono text-3xs font-medium text-secondary-foreground'>{unit}</span>
                    </span>
                </div>
                <div aria-hidden className='h-6.5 w-px bg-border' />
                <div className='flex flex-col gap-1.5'>
                    <div data-testid='pulls-trend-skeleton' aria-hidden className='flex h-7.5 w-29.5 items-end gap-0.75'>
                        {[40, 65, 30, 80, 55, 70, 45, 90, 35, 60, 75, 50].map((height, index) => (
                            <span key={index} className='flex-1 rounded-[1px] bg-secondary' style={{ height: `${height}%` }} />
                        ))}
                    </div>
                    <span className='font-mono text-3xs font-bold text-muted-foreground'>PULLS · 14d</span>
                </div>
            </div>
        </section>
    );
}
