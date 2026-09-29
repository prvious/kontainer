import { Plus, ScrollText, X } from "lucide-react";
import { StatusDot, type Tone } from "@/components/kit/badges";
import { LabelTooltip, TooltipRich } from "@/components/kit/tooltips";
import { TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { LogStatus } from "@/components/logs/log-buffer";
import type { LogTab } from "@/contexts/log-streams-context-value";
import { useLogBuffer } from "@/hooks/use-log-tabs";

const STATUS_TONE: Record<LogStatus, Tone> = { connecting: "neutral", live: "success", reconnecting: "warning", ended: "destructive" };

/** Why an ended stream stopped, for the tab's tooltip. */
function endedReason(ended: NonNullable<ReturnType<LogTab["buffer"]["getSnapshot"]>["ended"]>): string {
    const why = ended.reason === "service_removed" ? "The service was removed." : ended.message || `The stream ended (${ended.code}).`;
    return `${why} Lines received so far are kept; RESUME opens it again.`;
}

function TabHandle({ tab, name, onClose }: { tab: LogTab; name: string; onClose: () => void }) {
    const snapshot = useLogBuffer(tab.buffer);
    const trigger = (
        <TabsTrigger
            value={tab.id}
            className='h-9.5 flex-none gap-2 rounded-none border-0 px-0 font-mono text-2xs font-medium text-secondary-foreground data-active:text-sidebar-accent-foreground'
        >
            <StatusDot tone={STATUS_TONE[snapshot.status]} data-testid={`log-tab-dot-${name}`} className='size-1.5' />
            <ScrollText aria-hidden className='size-3.5 text-muted-foreground' />
            {name}
        </TabsTrigger>
    );
    return (
        <div className='flex h-9.5 items-center gap-2.5 border-r border-border px-4 has-[[data-active]]:border-b-2 has-[[data-active]]:border-b-primary'>
            {snapshot.ended ? (
                <TooltipRich title='Stream stopped' reason={endedReason(snapshot.ended)} side='bottom'>
                    {trigger}
                </TooltipRich>
            ) : (
                trigger
            )}
            <LabelTooltip label={`Close ${name}`}>
                <button type='button' aria-label={`Close ${name}`} onClick={onClose} className='rounded-sm text-muted-foreground hover:text-foreground'>
                    <X aria-hidden className='size-3' />
                </button>
            </LabelTooltip>
        </div>
    );
}

/** Log tabs `cNE8S`: service name, a live / reconnecting / ended dot, close, and `+`. */
export function LogTabs({
    tabs,
    nameOf,
    onClose,
    onNew,
}: {
    tabs: LogTab[];
    nameOf: (serviceId: string) => string;
    onClose: (id: string) => void;
    onNew: () => void;
}) {
    return (
        <div className='flex h-9.5 items-center border-b border-border'>
            <TabsList variant='line' className='h-9.5 gap-0 rounded-none p-0'>
                {tabs.map((tab) => (
                    <TabHandle key={tab.id} tab={tab} name={nameOf(tab.serviceId)} onClose={() => onClose(tab.id)} />
                ))}
            </TabsList>
            <LabelTooltip label='New stream'>
                <button
                    type='button'
                    aria-label='New stream'
                    onClick={onNew}
                    className='flex h-9.5 w-10 items-center justify-center text-muted-foreground hover:text-foreground'
                >
                    <Plus aria-hidden className='size-3.5' />
                </button>
            </LabelTooltip>
        </div>
    );
}
