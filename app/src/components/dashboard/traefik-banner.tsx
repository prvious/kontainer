import { Route, ArrowRight } from "lucide-react";

export function TraefikBanner() {
    return (
        <div className='flex items-center justify-between border border-border bg-card px-4 py-3'>
            <div className='flex items-center gap-2.5'>
                <Route className='size-4 text-accent-cyan' />
                <span className='text-[13px] font-semibold text-foreground'>Traefik Proxy</span>
                <span className='size-1.5 rounded-full bg-muted-foreground' />
                <span className='font-mono text-[10px] font-semibold text-muted-foreground'>--</span>
            </div>
            <div className='flex items-center gap-4'>
                <span className='font-mono text-[11px] text-muted-foreground'>-- req/s</span>
                <span className='font-mono text-[11px] text-muted-foreground'>-- p95</span>
                <span className='font-mono text-[11px] text-muted-foreground'>-- err</span>
                <ArrowRight className='size-3.5 text-muted-foreground' />
            </div>
        </div>
    );
}
