import type { ComponentProps } from "react";
import { cva } from "class-variance-authority";
import { Loader2, Lock, type LucideIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { TooltipRich } from "@/components/kit/tooltips";
import { PAUSED_OFFLINE } from "@/contexts/connectivity-context-value";
import { useConnectivity } from "@/hooks/use-connectivity";
import { cn } from "@/lib/utils";

/**
 * Buttons from Foundations CONTROLS: `z5Rrz` primary, `BSMVd` secondary, `qXPxh` outline, `jeLSJ`
 * compact primary. Labels are caps mono and bold.
 */
const actionButtonVariants = cva(
    "rounded-md border font-mono tracking-[0.5px] uppercase shadow-none select-none focus-visible:ring-2 focus-visible:ring-ring/50 disabled:opacity-40",
    {
        variants: {
            variant: {
                primary: "border-primary bg-primary font-bold text-primary-foreground hover:bg-primary/90",
                secondary: "border-border bg-transparent font-semibold text-secondary-foreground hover:bg-secondary hover:text-foreground",
                outline:
                    "border-border bg-transparent font-semibold text-secondary-foreground hover:bg-secondary hover:text-foreground [&_svg]:text-muted-foreground",
                destructive: "border-destructive bg-destructive font-bold text-destructive-foreground hover:bg-destructive/90",
                ghost: "border-transparent bg-transparent font-semibold text-secondary-foreground hover:bg-secondary hover:text-foreground [&_svg]:text-muted-foreground",
            },
            size: {
                md: "h-9 gap-2 px-4 text-xs [&_svg:not([class*='size-'])]:size-3.5",
                sm: "h-6.25 gap-1.5 px-3 text-3xs [&_svg:not([class*='size-'])]:size-3",
            },
        },
        defaultVariants: { variant: "outline", size: "sm" },
    },
);

export type ActionButtonVariant = "primary" | "secondary" | "outline" | "destructive" | "ghost";

export interface ActionButtonProps extends Omit<ComponentProps<typeof Button>, "variant" | "size"> {
    variant?: ActionButtonVariant;
    size?: "md" | "sm";
    icon?: LucideIcon;
    /** Keeps its width, shows a spinner, and swallows clicks so a submit cannot fire twice. */
    pending?: boolean;
    /** Shown with a lock and a Tooltip Rich explaining why the action is unavailable. */
    restricted?: { title: string; reason: string };
    /** Changes cluster state: restricted while the controller is unreachable (03.S2). */
    mutates?: boolean;
}

export function ActionButton({
    variant,
    size,
    icon: Icon,
    pending = false,
    restricted: declared,
    mutates = false,
    className,
    children,
    onClick,
    ...props
}: ActionButtonProps) {
    const { reachable } = useConnectivity();
    const restricted = declared ?? (mutates && !reachable ? PAUSED_OFFLINE : undefined);
    const LeadingIcon = restricted ? Lock : pending ? Loader2 : Icon;
    const blocked = pending || restricted !== undefined;
    const button = (
        <Button
            {...props}
            aria-busy={pending || undefined}
            aria-disabled={blocked || props.disabled || undefined}
            data-pending={pending || undefined}
            data-restricted={restricted ? true : undefined}
            className={cn(
                actionButtonVariants({ variant, size }),
                pending && "cursor-progress",
                restricted && "cursor-not-allowed opacity-60 hover:bg-transparent",
                className,
            )}
            onClick={(event) => {
                if (blocked) {
                    event.preventDefault();
                    return;
                }
                onClick?.(event);
            }}
        >
            {LeadingIcon && <LeadingIcon aria-hidden className={cn(pending && "animate-spin")} />}
            {children}
        </Button>
    );
    return restricted ? (
        <TooltipRich title={restricted.title} reason={restricted.reason}>
            {button}
        </TooltipRich>
    ) : (
        button
    );
}
