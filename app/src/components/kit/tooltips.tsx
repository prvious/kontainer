import type { ReactElement, ReactNode } from "react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

type Side = "top" | "bottom" | "left" | "right";

/** Tooltip `rXjCV`: a short label (and optional shortcut) for icon-only actions and truncated values. */
export function LabelTooltip({ label, shortcut, children }: { label: string; shortcut?: string; children: ReactElement }) {
    return (
        <Tooltip>
            <TooltipTrigger render={children} />
            <TooltipContent className='flex items-center gap-2.5 px-2.5 py-1.5 font-mono text-2xs font-medium'>
                <span>{label}</span>
                {shortcut && <span className='text-3xs font-semibold text-foreground-faint'>{shortcut}</span>}
            </TooltipContent>
        </Tooltip>
    );
}

/** Tooltip Rich `vNg0C`: title plus the reason an action is unavailable. */
export function TooltipRich({ title, reason, side = "top", children }: { title: string; reason: ReactNode; side?: Side; children: ReactElement }) {
    return (
        <Tooltip>
            <TooltipTrigger render={children} />
            <TooltipContent side={side} className='flex max-w-62 flex-col items-start gap-1 px-2.75 py-2.25 font-mono'>
                <span className='text-2xs font-bold'>{title}</span>
                <span className='text-3xs leading-normal text-foreground-faint'>{reason}</span>
            </TooltipContent>
        </Tooltip>
    );
}
