import { Plus, SquareTerminal, X } from "lucide-react";
import { StatusDot, type Tone } from "@/components/kit/badges";
import { LabelTooltip, TooltipRich } from "@/components/kit/tooltips";
import { TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { ExecStatus } from "@/components/exec/exec-buffer";
import type { ExecTab } from "@/contexts/exec-sessions-context-value";
import { useExecBuffer } from "@/hooks/use-exec-sessions";
import { MAX_EXEC_SESSIONS } from "@/types/exec";

const STATUS_TONE: Record<ExecStatus, Tone> = { connecting: "warning", active: "success", ended: "destructive", failed: "neutral" };

function TabHandle({ tab, onClose }: { tab: ExecTab; onClose: () => void }) {
    const snapshot = useExecBuffer(tab.buffer);
    return (
        <div className='flex h-9.5 items-center gap-2.5 border-r border-border px-4 has-[[data-active]]:border-b-2 has-[[data-active]]:border-b-primary'>
            <TabsTrigger
                value={tab.id}
                aria-keyshortcuts='Delete'
                onKeyDown={(event) => {
                    if (event.key === "Delete") onClose();
                }}
                className='h-9.5 flex-none gap-2 rounded-none border-0 px-0 font-mono text-2xs font-medium text-secondary-foreground data-active:text-sidebar-accent-foreground'
            >
                <StatusDot tone={STATUS_TONE[snapshot.status]} data-testid={`exec-tab-dot-${tab.name}`} className='size-1.5' />
                <SquareTerminal aria-hidden className='size-3.5 text-muted-foreground' />
                {tab.name}
            </TabsTrigger>
            {/* Out of the tab order: keyboard users close the focused tab with Delete. */}
            <LabelTooltip label={`Close ${tab.name}`}>
                <button
                    type='button'
                    tabIndex={-1}
                    aria-label={`Close ${tab.name}`}
                    onClick={onClose}
                    className='rounded-sm text-muted-foreground hover:text-foreground'
                >
                    <X aria-hidden className='size-3' />
                </button>
            </LabelTooltip>
        </div>
    );
}

/** Exec tabs `osf25`: container name, an active / ended dot, close, and `+` (off at four shells). */
export function ExecTabs({ tabs, open, onClose, onNew }: { tabs: ExecTab[]; open: number; onClose: (id: string) => void; onNew: () => void }) {
    const full = open >= MAX_EXEC_SESSIONS;
    const plus = (
        <button
            type='button'
            aria-label='New shell'
            aria-disabled={full || undefined}
            onClick={() => !full && onNew()}
            className='flex h-9.5 w-10 items-center justify-center text-muted-foreground hover:text-foreground aria-disabled:cursor-not-allowed aria-disabled:opacity-40'
        >
            <Plus aria-hidden className='size-3.5' />
        </button>
    );
    return (
        <div className='flex h-9.5 items-center border-b border-border'>
            <TabsList variant='line' className='h-9.5 gap-0 rounded-none p-0'>
                {tabs.map((tab) => (
                    <TabHandle key={tab.id} tab={tab} onClose={() => onClose(tab.id)} />
                ))}
            </TabsList>
            {full ? (
                <TooltipRich
                    title={`Session limit reached · ${MAX_EXEC_SESSIONS} of ${MAX_EXEC_SESSIONS} open`}
                    reason='Close one to start another. Idle shells close on their own after 30 min.'
                    side='bottom'
                >
                    {plus}
                </TooltipRich>
            ) : (
                <LabelTooltip label='New shell'>{plus}</LabelTooltip>
            )}
        </div>
    );
}
