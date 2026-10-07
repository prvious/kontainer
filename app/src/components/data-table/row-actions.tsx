import type { LucideIcon } from "lucide-react";
import { EllipsisVertical } from "lucide-react";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { LabelTooltip, TooltipRich } from "@/components/kit/tooltips";
import { PAUSED_OFFLINE } from "@/contexts/connectivity-context-value";
import { useConnectivity } from "@/hooks/use-connectivity";
import { cn } from "@/lib/utils";

export interface RowIconAction {
    label: string;
    icon: LucideIcon;
    onClick: () => void;
    disabled?: boolean;
    /** Why the action is unavailable; shown as a Tooltip Rich instead of the plain label. */
    disabledReason?: { title: string; reason: string };
    /** Changes cluster state: disabled while the controller is unreachable. */
    mutates?: boolean;
}

export interface RowMenuItem {
    label: string;
    onClick: () => void;
    disabled?: boolean;
    destructive?: boolean;
    separatorBefore?: boolean;
    /** Changes cluster state: disabled while the controller is unreachable. */
    mutates?: boolean;
}

/** RowActions `KPhXT` / Table Actions `Ulm5I`: icon buttons with 400 ms label tooltips, then a `⋮` menu. */
export function RowActions({
    label,
    actions: declaredActions = [],
    menu: declaredMenu = [],
}: {
    label: string;
    actions?: RowIconAction[];
    menu?: RowMenuItem[];
}) {
    const { reachable } = useConnectivity();
    const offline = (entry: { mutates?: boolean; disabled?: boolean }) => !reachable && entry.mutates === true && !entry.disabled;
    const actions = declaredActions.map((action) => (offline(action) ? { ...action, disabled: true, disabledReason: PAUSED_OFFLINE } : action));
    const menu = declaredMenu.map((item) => (offline(item) ? { ...item, disabled: true } : item));
    const paused = !reachable && declaredMenu.some((item) => item.mutates);
    return (
        <div className='flex items-center justify-end gap-2'>
            {actions.map((action) => {
                const button = (
                    <button
                        type='button'
                        aria-label={action.label}
                        aria-disabled={action.disabled || undefined}
                        onClick={() => {
                            if (!action.disabled) action.onClick();
                        }}
                        className={cn(
                            "flex size-5 items-center justify-center rounded-sm text-muted-foreground hover:bg-secondary hover:text-foreground",
                            action.disabled && "cursor-not-allowed opacity-40 hover:bg-transparent",
                        )}
                    >
                        <action.icon aria-hidden className='size-3.5' />
                    </button>
                );
                return action.disabled && action.disabledReason ? (
                    <TooltipRich key={action.label} title={action.disabledReason.title} reason={action.disabledReason.reason}>
                        {button}
                    </TooltipRich>
                ) : (
                    <LabelTooltip key={action.label} label={action.label}>
                        {button}
                    </LabelTooltip>
                );
            })}
            {menu.length > 0 && (
                <DropdownMenu>
                    <LabelTooltip label='More actions'>
                        <DropdownMenuTrigger
                            aria-label={`More actions for ${label}`}
                            className='flex size-5 items-center justify-center rounded-sm text-muted-foreground hover:bg-secondary hover:text-foreground'
                        >
                            <EllipsisVertical aria-hidden className='size-3.5' />
                        </DropdownMenuTrigger>
                    </LabelTooltip>
                    <DropdownMenuContent align='end' className='min-w-40'>
                        {paused && <p className='max-w-56 px-1.5 py-1 font-mono text-3xs leading-normal text-warning'>{PAUSED_OFFLINE.title}</p>}
                        {menu.map((item) => (
                            <MenuEntry key={item.label} item={item} />
                        ))}
                    </DropdownMenuContent>
                </DropdownMenu>
            )}
        </div>
    );
}

function MenuEntry({ item }: { item: RowMenuItem }) {
    return (
        <>
            {item.separatorBefore && <DropdownMenuSeparator />}
            <DropdownMenuItem
                disabled={item.disabled}
                variant={item.destructive ? "destructive" : "default"}
                onClick={item.onClick}
                className='font-mono text-2xs'
            >
                {item.label}
            </DropdownMenuItem>
        </>
    );
}
