import type { ReactNode } from "react";
import { Copy, PackageOpen, RefreshCw, ServerCrash, type LucideIcon } from "lucide-react";
import { toast } from "sonner";
import { ActionButton } from "@/components/kit/action-button";
import { errorDetailsText, type ErrorDetails } from "@/components/kit/error-details";
import { cn } from "@/lib/utils";

export interface RegionAction {
    label: string;
    icon?: LucideIcon;
    onClick?: () => void;
    disabled?: boolean;
    /** Plain tooltip for a deferred action ("Not built yet"). */
    restricted?: { title: string; reason: string };
}

/** Empty State `SJK65`: tile icon, title, body, and up to two actions. */
export function EmptyState({
    icon: Icon = PackageOpen,
    title,
    children,
    primaryAction,
    secondaryAction,
    className,
}: {
    icon?: LucideIcon;
    title: string;
    children?: ReactNode;
    primaryAction?: RegionAction;
    secondaryAction?: RegionAction;
    className?: string;
}) {
    return (
        <div role='status' className={cn("flex flex-col items-center gap-3.5 px-6 py-8 text-center", className)}>
            <div className='flex size-11 items-center justify-center rounded-lg border border-border bg-secondary'>
                <Icon aria-hidden className='size-5 text-muted-foreground' />
            </div>
            <h3 className='font-sans text-base font-semibold text-foreground'>{title}</h3>
            {children && <p className='max-w-85 font-mono text-2xs leading-relaxed text-muted-foreground'>{children}</p>}
            {(primaryAction || secondaryAction) && (
                <div className='flex items-center gap-2'>
                    {primaryAction && (
                        <ActionButton
                            variant='primary'
                            icon={primaryAction.icon}
                            onClick={primaryAction.onClick}
                            disabled={primaryAction.disabled}
                            restricted={primaryAction.restricted}
                        >
                            {primaryAction.label}
                        </ActionButton>
                    )}
                    {secondaryAction && (
                        <ActionButton
                            variant='outline'
                            icon={secondaryAction.icon}
                            onClick={secondaryAction.onClick}
                            disabled={secondaryAction.disabled}
                            restricted={secondaryAction.restricted}
                        >
                            {secondaryAction.label}
                        </ActionButton>
                    )}
                </div>
            )}
        </div>
    );
}

/** Error State `X7KWf`: a region that failed to load. Always shows the code and request id. */
export function ErrorState({
    title,
    children,
    details,
    onRetry,
    retryLabel = "TRY AGAIN",
    extra,
    className,
}: {
    title: string;
    children?: ReactNode;
    details: ErrorDetails;
    onRetry?: () => void;
    retryLabel?: string;
    extra?: ReactNode;
    className?: string;
}) {
    async function copyDetails() {
        try {
            await navigator.clipboard.writeText(errorDetailsText(details));
            toast.success("Details copied");
        } catch {
            toast.error("Couldn't copy details");
        }
    }

    return (
        <div role='alert' className={cn("flex flex-col items-center gap-3.5 px-6 py-8 text-center", className)}>
            <div className='flex size-11 items-center justify-center rounded-lg bg-destructive-soft'>
                <ServerCrash aria-hidden className='size-5 text-destructive' />
            </div>
            <h3 className='font-sans text-base font-semibold text-foreground'>{title}</h3>
            {children && <div className='max-w-85 font-mono text-2xs leading-relaxed text-muted-foreground'>{children}</div>}
            <div className='flex items-center gap-2 rounded-sm border border-border bg-muted px-2.5 py-1.5 font-mono text-3xs'>
                <span className='font-semibold text-destructive'>
                    {details.status ? `${details.status} ` : ""}
                    {details.code}
                </span>
                {details.requestId && <span className='text-muted-foreground'>req {details.requestId}</span>}
            </div>
            {extra}
            <div className='flex items-center gap-2'>
                {onRetry && (
                    <ActionButton variant='outline' icon={RefreshCw} onClick={onRetry}>
                        {retryLabel}
                    </ActionButton>
                )}
                <ActionButton variant='ghost' icon={Copy} onClick={() => void copyDetails()}>
                    COPY DETAILS
                </ActionButton>
            </div>
        </div>
    );
}
