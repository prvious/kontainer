import { ActionButton } from "@/components/kit/action-button";
import type { SessionErrorCode } from "@/types/session";

const messages: Record<SessionErrorCode, string> = {
    invalid_origin: "Enter a valid agent URL.",
    invalid_credentials: "Email or password is incorrect.",
    login_locked: "Sign-in is locked for this email. Try again later.",
    owner_exists: "This agent already has its owner. Sign in instead.",
    validation_failed: "Some fields need attention.",
    controller_unavailable: "The agent is unavailable. Check the address and try again.",
    invalid_response: "The agent returned an invalid response. Try again.",
    profile_conflict: "This desktop already has a different session. Sign out before connecting another agent.",
    relogin_required: "Your session expired or was revoked. Sign in again.",
    vault_unavailable: "The system credential vault is unavailable. Unlock it and retry.",
    vault_write_failed: "Your session was issued but could not be saved to the credential vault.",
    local_state_failed: "Local session state could not be read or saved. Retry after checking local storage.",
    revoke_failed: "The agent could not confirm sign-out. Your session is still held; retry sign-out.",
    local_cleanup_failed: "The agent session was revoked, but local cleanup failed. Retry cleanup.",
    cleanup_uncertain: "Session cleanup could not be confirmed. Retry saving or signing out before signing in again.",
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
        <div
            role='alert'
            className='flex flex-col gap-3 rounded-sm border-l-2 border-l-destructive bg-destructive-soft px-3.5 py-2.5 font-mono text-2xs text-foreground'
        >
            <p>{messages[code]}</p>
            <div className='flex flex-wrap gap-2'>
                {(code === "vault_write_failed" || code === "cleanup_uncertain") && onRetryCommit && (
                    <ActionButton variant='outline' onClick={onRetryCommit}>
                        Retry saving session
                    </ActionButton>
                )}
                {(code === "cleanup_uncertain" ||
                    code === "local_cleanup_failed" ||
                    code === "revoke_failed" ||
                    code === "profile_conflict" ||
                    code === "vault_write_failed") &&
                    onLogout && (
                        <ActionButton variant='outline' onClick={onLogout}>
                            Retry sign-out
                        </ActionButton>
                    )}
                {(code === "vault_unavailable" || code === "local_state_failed") && onRetryRestore && (
                    <ActionButton variant='outline' onClick={onRetryRestore}>
                        Retry restore
                    </ActionButton>
                )}
            </div>
        </div>
    );
}
