import { useEffect, useState } from "react";
import { CircleX, RefreshCw, ScrollText } from "lucide-react";
import { AccentLine } from "@/components/accent-line";
import { ActionButton } from "@/components/kit/action-button";
import { LogoMark } from "@/components/logo";
import { useAppVersion } from "@/hooks/use-app-version";
import type { SessionErrorCode } from "@/types/session";
import { cn } from "@/lib/utils";

/** Restore failures that the splash reports; the others have recovery actions on the sign-in screen. */
export const STARTUP_FAILURE_CODES = [
    "vault_unavailable",
    "local_state_failed",
    "controller_unavailable",
    "invalid_response",
] as const satisfies readonly SessionErrorCode[];
export type StartupFailureCode = (typeof STARTUP_FAILURE_CODES)[number] | "restore_timeout";

export const RESTORE_TIMEOUT_MS = 15_000;

const failureCopy: Record<StartupFailureCode, { stoppedAt: string; body: string }> = {
    vault_unavailable: {
        stoppedAt: "unlocking credential store",
        body: "Kontainer couldn't read your saved credentials from the OS keychain. The keychain may be locked, or access was denied when the system asked.",
    },
    local_state_failed: {
        stoppedAt: "reading local session state",
        body: "Kontainer couldn't read or save its local session file. Check that the app data folder is writable, then retry.",
    },
    controller_unavailable: {
        stoppedAt: "restoring session",
        body: "The controller didn't answer while restoring your session. Your saved session is kept; retry when the controller is reachable.",
    },
    invalid_response: {
        stoppedAt: "restoring session",
        body: "The controller answered with a response Kontainer doesn't understand. Check that the controller and the desktop app are on matching versions.",
    },
    restore_timeout: {
        stoppedAt: "restoring session",
        body: "Restoring the saved session took longer than 15 seconds. Nothing was changed; retry, or check the keychain prompt and the controller.",
    },
};

/** True once `ms` has passed since mount; the splash is keyed per restore attempt so this restarts. */
function useElapsed(ms: number) {
    const [elapsed, setElapsed] = useState(false);
    useEffect(() => {
        const timer = window.setTimeout(() => setElapsed(true), ms);
        return () => window.clearTimeout(timer);
    }, [ms]);
    return elapsed;
}

const GRID_LINES = ["top-1/3", "top-1/2", "top-2/3"];
const GRID_COLUMNS = ["left-1/4", "left-1/2", "left-3/4"];

/**
 * Splash `PwbGj` and state 02.S1 `gcAoz`. Never an endless spinner: after 15 s without a restore
 * answer, or on a startup failure, the bar turns destructive and an error card offers RETRY.
 */
export function SplashScreen({ failure, onRetry }: { failure?: StartupFailureCode | null; onRetry?: () => void }) {
    const version = useAppVersion();
    const timedOut = useElapsed(RESTORE_TIMEOUT_MS);
    const shown: StartupFailureCode | null = failure ?? (timedOut ? "restore_timeout" : null);
    const copy = shown ? failureCopy[shown] : null;

    return (
        <div className='relative flex h-screen flex-col overflow-hidden bg-background'>
            <AccentLine />
            {GRID_LINES.map((position) => (
                <div key={position} aria-hidden className={cn("absolute inset-x-0 h-px bg-secondary", position)} />
            ))}
            {GRID_COLUMNS.map((position) => (
                <div key={position} aria-hidden className={cn("absolute inset-y-0 w-px bg-secondary", position)} />
            ))}
            <span className='absolute top-5 left-6 font-mono text-3xs font-semibold tracking-[2px] text-foreground-faint'>KONTAINER.DEV</span>
            <span className='absolute right-6 bottom-6 font-mono text-3xs font-semibold tracking-[2px] text-foreground-faint'>DESKTOP APPLICATION</span>

            <div className='relative flex flex-1 items-center justify-center'>
                <div className='flex w-100 flex-col items-center gap-7'>
                    <div className='flex size-22 items-center justify-center rounded-lg border border-border bg-card'>
                        <LogoMark size='lg' />
                    </div>
                    <div className='flex flex-col items-center gap-2.5'>
                        <span className='font-brand text-4xl font-black tracking-[0.7px] text-foreground'>KONTAINER</span>
                        <span className='font-mono text-sm font-medium text-secondary-foreground'>Infrastructure, Simplified</span>
                    </div>
                    <div className='flex w-full flex-col items-center gap-3'>
                        <div className='h-0.75 w-70 overflow-hidden rounded-[2px] bg-secondary'>
                            <div className={cn("h-full rounded-[2px]", copy ? "w-[45%] bg-destructive" : "w-2/3 animate-pulse bg-chart-1")} />
                        </div>
                        {copy ? (
                            <span className='font-mono text-2xs font-medium text-destructive'>Stopped at: {copy.stoppedAt}</span>
                        ) : (
                            <span className='font-mono text-2xs font-medium text-muted-foreground'>Initializing runtime…</span>
                        )}
                    </div>
                    {copy && shown && (
                        <div role='alert' className='flex w-full flex-col gap-3 rounded-md border border-l-2 border-border border-l-destructive bg-card p-4'>
                            <div className='flex items-center gap-2.5'>
                                <CircleX aria-hidden className='size-4 text-destructive' />
                                <span className='font-mono text-xs font-semibold text-foreground'>Couldn't start the local runtime</span>
                            </div>
                            <p className='font-mono text-2xs leading-relaxed text-muted-foreground'>{copy.body}</p>
                            <span className='self-start rounded-sm border border-border bg-muted px-2.5 py-1.5 font-mono text-3xs font-semibold text-destructive'>
                                {shown}
                            </span>
                            <div className='flex items-center gap-2'>
                                <ActionButton variant='primary' icon={RefreshCw} onClick={onRetry}>
                                    RETRY
                                </ActionButton>
                                <ActionButton
                                    variant='outline'
                                    icon={ScrollText}
                                    restricted={{ title: "Not built yet", reason: "Desktop logs arrive in a later release." }}
                                >
                                    OPEN LOGS
                                </ActionButton>
                            </div>
                        </div>
                    )}
                    <span className='font-mono text-3xs font-medium text-muted-foreground'>{version ? `v${version}` : null}</span>
                </div>
            </div>
        </div>
    );
}
