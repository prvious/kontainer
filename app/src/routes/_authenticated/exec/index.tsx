import { useState } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { toast } from "sonner";
import { Plus, Terminal } from "lucide-react";
import { PageHeader } from "@/components/page-header";
import { ExecSessionTabs } from "@/components/exec/exec-session-tabs";
import { ContainerPickerDialog } from "@/components/exec/container-picker-dialog";
import { ExecConnectionBar } from "@/components/exec/exec-connection-bar";
import { TerminalSessionView } from "@/components/terminal-session-view";
import { useTerminal } from "@/hooks/use-terminal";
import { Button } from "@/components/ui/button";

const MAX_SESSIONS = 10;

function ExecPage() {
    const { sessions, activeSessionId, openExec } = useTerminal();
    const [pickerOpen, setPickerOpen] = useState(false);

    function openPicker() {
        if (sessions.length >= MAX_SESSIONS) {
            toast.warning(`Maximum ${MAX_SESSIONS} sessions allowed. Close a tab first.`);
            return;
        }
        setPickerOpen(true);
    }

    function handleSelect(target: { containerId: string; nodeId: string; containerName: string }) {
        openExec({ containerId: target.containerId, nodeId: target.nodeId, containerName: target.containerName }, { showPanel: false });
        setPickerOpen(false);
    }

    const hasSessions = sessions.length > 0;

    return (
        <div className='flex h-full flex-col gap-4 p-6'>
            <PageHeader title='EXEC' subtitle='// interactive terminal sessions'>
                <button
                    onClick={openPicker}
                    className='flex items-center gap-1.5 rounded-md border border-accent-green bg-accent-green-20 px-3 py-1.5 font-mono text-[11px] font-semibold text-accent-green'
                >
                    <Plus className='size-3.5' />
                    New Session
                </button>
            </PageHeader>

            {hasSessions ? (
                <>
                    <ExecSessionTabs onAdd={openPicker} />
                    <div className='flex min-h-0 flex-1 flex-col overflow-hidden rounded-b-lg border-x border-b border-border'>
                        {sessions.map((session) => (
                            <div key={session.id} className='flex-1' style={{ display: session.id === activeSessionId ? "flex" : "none" }}>
                                <TerminalSessionView target={session.target} onStatusChange={() => {}} />
                            </div>
                        ))}
                    </div>
                    <ExecConnectionBar />
                </>
            ) : (
                <div className='flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground'>
                    <Terminal className='size-8' />
                    <p className='font-mono text-sm'>No terminal sessions</p>
                    <p className='font-mono text-xs'>Click &quot;New Session&quot; to exec into a container</p>
                    <Button variant='outline' size='sm' onClick={openPicker} className='mt-2 gap-1.5'>
                        <Plus className='size-3.5' />
                        New Session
                    </Button>
                </div>
            )}

            <ContainerPickerDialog
                open={pickerOpen}
                onOpenChange={setPickerOpen}
                onSelect={handleSelect}
                excludeContainerIds={sessions.map((s) => s.target.containerId)}
            />
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/exec/")({
    component: ExecPage,
});
