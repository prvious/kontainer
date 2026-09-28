import type { ReactNode } from "react";
import { CircleCheck, Info, ServerCrash, TriangleAlert, type LucideIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { ActionButton } from "@/components/kit/action-button";
import { cn } from "@/lib/utils";

export type AlertTone = "warning" | "destructive" | "info" | "success";

const toneClass: Record<AlertTone, string> = {
    warning: "border-l-warning bg-warning-soft [&>svg]:text-warning",
    destructive: "border-l-destructive bg-destructive-soft [&>svg]:text-destructive",
    info: "border-l-border bg-secondary [&>svg]:text-muted-foreground",
    success: "border-l-success bg-success-soft [&>svg]:text-success",
};

const toneIcon: Record<AlertTone, LucideIcon> = {
    warning: TriangleAlert,
    destructive: ServerCrash,
    info: Info,
    success: CircleCheck,
};

export interface InlineAlertAction {
    label: string;
    icon?: LucideIcon;
    onClick: () => void;
    pending?: boolean;
}

/** Alert `yDnxo`: a persistent condition at the top of the region it affects. */
export function InlineAlert({
    tone,
    title,
    children,
    action,
    className,
}: {
    tone: AlertTone;
    title: ReactNode;
    children?: ReactNode;
    action?: InlineAlertAction;
    className?: string;
}) {
    const Icon = toneIcon[tone];
    return (
        <Alert
            data-tone={tone}
            className={cn("flex items-center gap-3 rounded-sm border-0 border-l-2 px-3.5 py-2.5 [&>svg]:size-3.5 [&>svg]:shrink-0", toneClass[tone], className)}
        >
            <Icon aria-hidden />
            <div className='flex min-w-0 flex-1 flex-col gap-0.75'>
                <AlertTitle className='font-mono text-xs font-semibold text-foreground'>{title}</AlertTitle>
                {children && <AlertDescription className='font-mono text-2xs leading-normal text-secondary-foreground'>{children}</AlertDescription>}
            </div>
            {action && (
                <ActionButton variant='outline' className='shrink-0 bg-background' icon={action.icon} pending={action.pending} onClick={action.onClick}>
                    {action.label}
                </ActionButton>
            )}
        </Alert>
    );
}
