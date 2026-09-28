import { Button } from "@/components/ui/button";
import type { SessionErrorCode } from "@/types/session";

const messages: Record<SessionErrorCode, string> = {
    invalid_origin: "Enter a valid controller URL.",
    browser_open_failed: "The browser could not be opened. Try signing in again.",
    login_cancelled: "Sign-in was cancelled.",
    login_expired: "Sign-in expired. Start again.",
    login_denied: "GitHub sign-in was denied for this controller.",
    provider_unavailable: "GitHub is unavailable. Try again later.",
    controller_unavailable: "The controller is unavailable. Check the address and try again.",
    invalid_response: "The controller returned an invalid response. Try again.",
    profile_conflict: "This desktop already has a different session. Sign out before connecting another controller.",
    relogin_required: "Your session expired or was revoked. Sign in again.",
    vault_unavailable: "The system credential vault is unavailable. Unlock it and retry.",
    vault_write_failed: "Your session was issued but could not be saved to the credential vault.",
    local_state_failed: "Local session state could not be read or saved. Retry after checking local storage.",
    revoke_failed: "The controller could not confirm sign-out. Your session is still held; retry sign-out.",
    local_cleanup_failed: "The controller session was revoked, but local cleanup failed. Retry cleanup.",
    cleanup_uncertain: "Session cleanup could not be confirmed. Retry saving or signing out before starting another login.",
};

export function SessionIssue({
    code,
    onRetryCommit,
    onRetryRestore,
    onLogout,
}: {
    code: SessionErrorCode;
    onRetryCommit?: () => void;
    onRetryRestore?: () => void;
    onLogout?: () => void;
}) {
    return (
        <div role='alert' className='space-y-3 border border-destructive/50 bg-destructive/10 p-3 text-sm text-foreground'>
            <p>{messages[code]}</p>
            <div className='flex flex-wrap gap-2'>
                {(code === "vault_write_failed" || code === "cleanup_uncertain") && onRetryCommit && (
                    <Button size='sm' variant='outline' onClick={onRetryCommit}>
                        Retry saving session
                    </Button>
                )}
                {(code === "cleanup_uncertain" ||
                    code === "local_cleanup_failed" ||
                    code === "revoke_failed" ||
                    code === "profile_conflict" ||
                    code === "vault_write_failed") &&
                    onLogout && (
                        <Button size='sm' variant='outline' onClick={onLogout}>
                            Retry sign-out
                        </Button>
                    )}
                {(code === "vault_unavailable" || code === "local_state_failed") && onRetryRestore && (
                    <Button size='sm' variant='outline' onClick={onRetryRestore}>
                        Retry restore
                    </Button>
                )}
            </div>
        </div>
    );
}
