import { useState } from "react";
import { Play, RotateCw, ScrollText, SquareTerminal, X } from "lucide-react";
import { ActionButton } from "@/components/kit/action-button";
import { Input } from "@/components/ui/input";
import { TerminalView } from "@/components/exec/terminal-view";
import type { ExecSnapshot } from "@/components/exec/exec-buffer";
import type { ExecTab } from "@/contexts/exec-sessions-context-value";
import { useExecBuffer } from "@/hooks/use-exec-sessions";
import { clockTime } from "@/lib/time";
import { endedText } from "@/types/exec";

/** Why a shell didn't start, as the can't-start state titles it (08.S2). */
function failureTitle(code: string, name: string): string {
    switch (code) {
        case "target_not_running":
            return `${name} isn't running`;
        case "shell_not_found":
            return `No shell found in ${name}`;
        case "limit_reached":
            return "Session limit reached";
        default:
            return `Couldn't start a shell in ${name}`;
    }
}

function failureText(code: string, message: string): string {
    switch (code) {
        case "target_not_running":
            return "Start it to open a shell. Exec needs a running container; no session slot was used.";
        case "shell_not_found":
            return `${message} Name a shell the image has, such as /bin/ash or /usr/bin/zsh.`;
        case "limit_reached":
            return "Four shells are open for this sign-in. Close one to start another; idle shells close on their own after 30 minutes.";
        default:
            return message;
    }
}

function CantStart({
    tab,
    error,
    onStart,
    onViewLogs,
    onRetry,
    starting,
}: {
    tab: ExecTab;
    error: NonNullable<ExecSnapshot["error"]>;
    onStart: () => void;
    onViewLogs: (() => void) | null;
    onRetry: (shell: string | null) => void;
    starting: boolean;
}) {
    const [shell, setShell] = useState(tab.shell ?? "");
    return (
        <div role='alert' className='flex flex-1 flex-col items-center justify-center gap-3.5 px-6 py-10 text-center'>
            <div className='flex size-11 items-center justify-center rounded-lg bg-destructive-soft'>
                <SquareTerminal aria-hidden className='size-5 text-destructive' />
            </div>
            <h3 className='font-sans text-base font-semibold text-foreground'>{failureTitle(error.code, tab.name)}</h3>
            <p className='max-w-96 font-mono text-2xs leading-relaxed text-muted-foreground'>{failureText(error.code, error.message)}</p>
            <span className='rounded-sm border border-border bg-secondary px-2 py-1 font-mono text-3xs text-destructive'>{error.code}</span>
            {error.code === "target_not_running" ? (
                <div className='flex items-center gap-2'>
                    <ActionButton variant='outline' icon={Play} mutates pending={starting} onClick={onStart}>
                        START CONTAINER
                    </ActionButton>
                    {onViewLogs && (
                        <ActionButton variant='ghost' icon={ScrollText} onClick={onViewLogs}>
                            VIEW LOGS
                        </ActionButton>
                    )}
                </div>
            ) : error.code === "shell_not_found" ? (
                <form
                    className='flex items-center gap-2'
                    onSubmit={(event) => {
                        event.preventDefault();
                        if (shell.trim()) onRetry(shell.trim());
                    }}
                >
                    <Input
                        aria-label='Shell to run'
                        placeholder='/bin/ash'
                        value={shell}
                        onChange={(event) => setShell(event.target.value)}
                        className='h-7.5 w-48 font-mono text-2xs'
                    />
                    <ActionButton type='submit' variant='primary' icon={RotateCw} disabled={!shell.trim()}>
                        RETRY
                    </ActionButton>
                </form>
            ) : error.code === "limit_reached" ? null : (
                <ActionButton variant='outline' icon={RotateCw} onClick={() => onRetry(tab.shell)}>
                    RETRY
                </ActionButton>
            )}
        </div>
    );
}

/**
 * One Exec tab: the terminal while the shell runs; after it ends, the same output dimmed with a
 * divider and RECONNECT / CLOSE TAB (08.S1); or the can't-start state (08.S2).
 */
export function ExecTabBody({
    tab,
    onData,
    onResize,
    onReconnect,
    onClose,
    onStart,
    onViewLogs,
    starting,
}: {
    tab: ExecTab;
    onData: (data: string) => void;
    onResize: (cols: number, rows: number) => void;
    onReconnect: (shell?: string | null) => void;
    onClose: () => void;
    onStart: () => void;
    onViewLogs: (() => void) | null;
    starting: boolean;
}) {
    const snapshot = useExecBuffer(tab.buffer);
    if (snapshot.status === "failed" && snapshot.error) {
        return (
            <CantStart tab={tab} error={snapshot.error} onStart={onStart} onViewLogs={onViewLogs} onRetry={(shell) => onReconnect(shell)} starting={starting} />
        );
    }
    const ended = snapshot.status === "ended";
    const why = snapshot.error ? snapshot.error.code.split("_").join(" ") : endedText(snapshot.exit, snapshot.closed);
    return (
        <div className='flex min-h-0 flex-1 flex-col bg-[#08090b]'>
            <div className={ended ? 'min-h-0 flex-[2]' : 'min-h-0 flex-1'}>
                <TerminalView tab={tab} ended={ended} onData={onData} onResize={onResize} />
            </div>
            {ended && (
                <div className='flex flex-col gap-3 px-4 pb-4'>
                    <div role='separator' className='flex items-center gap-3 font-mono text-3xs text-destructive'>
                        <span className='h-px flex-1 bg-destructive/30' />
                        {why}
                        {snapshot.endedAt !== null && ` · ${clockTime(snapshot.endedAt, true)}`}
                        {(snapshot.exit === null || (snapshot.exit.code === null && snapshot.exit.reason !== "exited")) && " · exit code unknown"}
                        <span className='h-px flex-1 bg-destructive/30' />
                    </div>
                    <div role='status' className='flex flex-col gap-2 rounded-sm border-l-2 border-destructive bg-destructive-soft px-4 py-3'>
                        <span className='font-mono text-xs font-semibold text-foreground'>Session ended — {why}</span>
                        <span className='font-mono text-2xs text-secondary-foreground'>
                            Output above is kept read-only. Reconnect opens a fresh shell in {tab.name}.
                        </span>
                        <div className='flex items-center gap-2'>
                            <ActionButton variant='primary' icon={RotateCw} onClick={() => onReconnect()}>
                                RECONNECT
                            </ActionButton>
                            <ActionButton variant='outline' icon={X} onClick={onClose}>
                                CLOSE TAB
                            </ActionButton>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
