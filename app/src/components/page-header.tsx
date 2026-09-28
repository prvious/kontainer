import type { ReactNode } from "react";

/** Page Header from `WCBzg`: caps title, mono subtitle line, and a right-side actions slot. */
export function PageHeader({ title, subtitle, children }: { title: string; subtitle: ReactNode; children?: ReactNode }) {
    return (
        <div className='flex items-center justify-between gap-4'>
            <div className='flex min-w-0 flex-col gap-1.75'>
                <h1 className='font-sans text-2xl font-bold tracking-[0.4px] text-foreground uppercase'>{title}</h1>
                <p className='truncate font-mono text-2xs font-medium tracking-[0.3px] text-secondary-foreground'>{subtitle}</p>
            </div>
            {children && <div className='flex shrink-0 items-center gap-2.5'>{children}</div>}
        </div>
    );
}
