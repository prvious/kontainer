import { useState, type FormEvent } from "react";
import { Info, KeyRound, LockKeyhole, LogIn } from "lucide-react";
import { useNavigate } from "@tanstack/react-router";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { ActionButton } from "@/components/kit/action-button";
import { ConnectField } from "@/components/connect/connect-field";
import { connectionHost, invalidCredentialsMessage, lockedMessage } from "@/components/connect/connect-rules";
import { useAuth } from "@/hooks/use-auth";
import { isLoginFailure } from "@/types/session";
import { clockTime } from "@/lib/time";

/**
 * State 02.S5 `ME9dW`: the refresh was rejected. The page stays mounted and inert under the
 * scrim; signing in again with the same email resumes it with its filters.
 */
export function SessionExpiredDialog() {
    const { session, expired, login, logout } = useAuth();
    const navigate = useNavigate();
    const [password, setPassword] = useState("");
    const [error, setError] = useState<string | undefined>();
    const [pending, setPending] = useState(false);
    const [expiredAt] = useState(() => Date.now());

    if (!session || !expired) return null;
    const name = session.label ?? connectionHost(session.origin);
    const origin = session.origin;

    async function signIn(event: FormEvent) {
        event.preventDefault();
        if (!session || pending) return;
        setPending(true);
        try {
            await login({ origin: session.origin, email: session.email, password, label: session.label });
            setPassword("");
            setError(undefined);
        } catch (failure) {
            setPassword("");
            if (isLoginFailure(failure)) {
                setError(
                    failure.code === "invalid_credentials"
                        ? invalidCredentialsMessage(failure)
                        : failure.code === "login_locked"
                          ? lockedMessage(failure, Date.now())
                          : `Couldn't sign in (${failure.code}).`,
                );
            }
        } finally {
            setPending(false);
        }
    }

    async function switchAgent() {
        // The session is already invalid on the controller; sign-out clears what custody still holds.
        await logout().catch(() => {});
        void navigate({ to: "/login", search: { redirect: undefined, origin } });
    }

    return (
        <Dialog open modal disablePointerDismissal>
            <DialogContent showCloseButton={false} className='w-115 max-w-[calc(100vw-2rem)] gap-0 rounded-lg bg-popover p-0 ring-border sm:max-w-115'>
                <form onSubmit={(event) => void signIn(event)}>
                    <div className='flex gap-3.5 px-5 pt-5'>
                        <div className='flex size-9 shrink-0 items-center justify-center rounded-md bg-primary-soft text-primary'>
                            <KeyRound aria-hidden className='size-4' />
                        </div>
                        <div className='flex min-w-0 flex-col gap-1.5'>
                            <DialogTitle className='font-sans text-base font-semibold text-foreground'>Your session ended</DialogTitle>
                            <DialogDescription className='font-mono text-2xs leading-relaxed text-muted-foreground'>
                                You were signed out of {name}. Sign in again to pick up where you left off — you'll come back to this page with the same
                                filters.
                            </DialogDescription>
                        </div>
                    </div>
                    <div className='flex flex-col gap-4 pt-4 pr-5 pl-17.5'>
                        <div className='flex items-center gap-2.5 rounded-sm bg-secondary px-3 py-2.5 font-mono text-2xs text-secondary-foreground'>
                            <Info aria-hidden className='size-3.25 shrink-0 text-muted-foreground' />
                            Reason: the controller rejected the session (signed out elsewhere, expired, or revoked) · {clockTime(expiredAt)}
                        </div>
                        <p className='font-mono text-2xs text-muted-foreground'>
                            Signing in as <span className='text-foreground'>{session.email}</span>
                        </p>
                        <ConnectField
                            id='expired-password'
                            label='Password'
                            icon={LockKeyhole}
                            secret
                            autoFocus
                            value={password}
                            error={error}
                            onChange={(event) => setPassword(event.target.value)}
                        />
                    </div>
                    <div className='mt-5 flex items-center justify-end gap-2 border-t border-border px-5 py-3.5'>
                        <ActionButton type='button' variant='secondary' className='h-7.75 px-3.5 text-2xs' onClick={() => void switchAgent()}>
                            SWITCH AGENT
                        </ActionButton>
                        <ActionButton type='submit' variant='primary' className='h-7.75 px-3.5 text-2xs' icon={LogIn} pending={pending} disabled={!password}>
                            SIGN IN
                        </ActionButton>
                    </div>
                </form>
            </DialogContent>
        </Dialog>
    );
}
