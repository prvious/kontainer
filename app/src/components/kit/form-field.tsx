import type { ReactNode } from "react";
import { CircleAlert } from "lucide-react";
import { fieldHelperId } from "@/components/kit/field-ids";

/**
 * Field Error pattern `JRLoZ`: label, control, and a helper line that turns destructive with a
 * `circle-alert` when invalid. The control sets `aria-invalid` and `aria-describedby={fieldHelperId(id)}`.
 * Validation runs on blur and on submit in the owning form.
 */
export function FormField({
    id,
    label,
    error,
    helper,
    children,
    trailing,
}: {
    id: string;
    label: string;
    error?: string | null;
    helper?: ReactNode;
    children: ReactNode;
    trailing?: ReactNode;
}) {
    return (
        <div className='flex flex-col gap-2'>
            <div className='flex items-center justify-between'>
                <label htmlFor={id} className='font-mono text-3xs font-medium tracking-[1px] text-muted-foreground uppercase'>
                    {label}
                </label>
                {trailing}
            </div>
            {children}
            {error ? (
                <p id={fieldHelperId(id)} role='alert' className='flex gap-1.5 font-mono text-2xs leading-normal text-destructive'>
                    <CircleAlert aria-hidden className='mt-0.5 size-3 shrink-0' />
                    <span>{error}</span>
                </p>
            ) : helper ? (
                <p id={fieldHelperId(id)} className='font-mono text-2xs leading-normal text-muted-foreground'>
                    {helper}
                </p>
            ) : null}
        </div>
    );
}
