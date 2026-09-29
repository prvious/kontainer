import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { fieldHelperId } from "@/components/kit/field-ids";
import { cn } from "@/lib/utils";

/** A Settings card: icon, caps title, an optional badge, then its rows. */
export function SettingsSection({ icon: Icon, title, badge, children }: { icon: LucideIcon; title: string; badge?: ReactNode; children: ReactNode }) {
    return (
        <section aria-label={title} className='overflow-hidden rounded-md border border-border bg-card'>
            <div className='flex h-12 items-center gap-3 px-5'>
                <Icon aria-hidden className='size-4 text-muted-foreground' />
                <h2 className='font-mono text-sm font-bold tracking-[1px] text-foreground uppercase'>{title}</h2>
                {badge && (
                    <span className='rounded-sm border border-border bg-secondary px-1.5 py-0.5 font-mono text-3xs text-secondary-foreground'>{badge}</span>
                )}
            </div>
            <div className='divide-y divide-border border-t border-border'>{children}</div>
        </section>
    );
}

/**
 * One row: label and mono description on the left, the control on the right. A staged change
 * tints the row and notes the old value; an invalid value tints it red with the reason under
 * the control (08.S5).
 */
export function SettingsRow({
    id,
    label,
    description,
    changedFrom,
    error,
    note,
    children,
}: {
    id: string;
    label: string;
    description: string;
    /** The stored value when this row has a staged change. */
    changedFrom?: string | null;
    error?: string | null;
    /** Extra line under the description, such as where a read-only value is set. */
    note?: ReactNode;
    children: ReactNode;
}) {
    return (
        <div
            data-changed={changedFrom ? true : undefined}
            data-invalid={error ? true : undefined}
            className={cn("flex items-center justify-between gap-6 px-5 py-4", error ? "bg-destructive-faint" : changedFrom && "bg-primary-faint")}
        >
            <div className='flex min-w-0 flex-col gap-1'>
                <label htmlFor={id} className='font-mono text-sm font-semibold text-foreground'>
                    {label}
                </label>
                <p className='font-mono text-2xs text-muted-foreground'>{description}</p>
                {changedFrom && !error && (
                    <p className='flex items-center gap-1.5 font-mono text-3xs font-medium text-sidebar-accent-foreground'>
                        <span aria-hidden className='size-1.25 rounded-full bg-primary' />
                        Changed from {changedFrom}
                    </p>
                )}
                {note}
            </div>
            <div className='flex shrink-0 flex-col items-end gap-1.5'>
                {children}
                {error && (
                    <p id={fieldHelperId(id)} role='alert' className='font-mono text-2xs text-destructive'>
                        {error}
                    </p>
                )}
            </div>
        </div>
    );
}
