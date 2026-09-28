import { useCallback, useState } from "react";
import { QueryClient, QueryClientProvider, useQueryClient } from "@tanstack/react-query";
import { AuthContext, type RegisterInput, type SignInInput } from "@/contexts/auth-context-value";
import { useSessionLogin } from "@/hooks/session/use-session-login";
import { useSessionLogout } from "@/hooks/session/use-session-logout";
import { useSessionRegister } from "@/hooks/session/use-session-register";
import { useSessionRestore, sessionRestoreKey } from "@/hooks/session/use-session-restore";
import { useSessionRetryCommit } from "@/hooks/session/use-session-retry-commit";
import { sessionConnectionsKey } from "@/hooks/session/use-session-connections";
import { isLoginFailure, sessionErrorCode, type ConnectionSummary, type SessionErrorCode, type SessionView } from "@/types/session";
import type { InventoryErrorCode } from "@/types/inventory";

/** Custody failures the shell surfaces with recovery actions (SessionIssue). */
const CUSTODY_CODES: readonly SessionErrorCode[] = [
    "vault_unavailable",
    "vault_write_failed",
    "local_state_failed",
    "cleanup_uncertain",
    "profile_conflict",
    "revoke_failed",
    "local_cleanup_failed",
];

function SessionAuthProvider({ children }: { children: React.ReactNode }) {
    const queryClient = useQueryClient();
    const restore = useSessionRestore();
    const loginCommand = useSessionLogin();
    const registerCommand = useSessionRegister();
    const logoutCommand = useSessionLogout();
    const retryCommitCommand = useSessionRetryCommit();
    const [actionError, setActionError] = useState<SessionErrorCode | null>(null);
    const [expired, setExpired] = useState(false);

    const session = restore.data ?? null;
    const issue = actionError ?? (restore.error ? sessionErrorCode(restore.error) : null);

    const publishSession = useCallback(
        (view: SessionView | null) => {
            const current = queryClient.getQueryData<SessionView | null>(sessionRestoreKey);
            // Signing back in to the same profile (after 02.S5) keeps the page's data; any other
            // change drops profile-scoped queries. App-level data (["app", ...]) always stays.
            if (!view || current?.profileId !== view.profileId) {
                queryClient.removeQueries({ predicate: (query) => query.queryKey[0] !== "session" && query.queryKey[0] !== "app" });
                queryClient.getMutationCache().clear();
            }
            queryClient.setQueryData(sessionRestoreKey, view);
            void queryClient.invalidateQueries({ queryKey: sessionConnectionsKey });
            setExpired(false);
        },
        [queryClient],
    );

    const reportInventoryCustodyError = useCallback(
        (profileId: string, code: InventoryErrorCode) => {
            const current = queryClient.getQueryData<SessionView | null>(sessionRestoreKey);
            if (current?.profileId !== profileId) return;
            if (code === "unauthenticated" || code === "relogin_required") {
                // 02.S5: keep the page mounted and inert; the dialog signs back in.
                setExpired(true);
                return;
            }
            if (CUSTODY_CODES.includes(code as SessionErrorCode)) {
                publishSession(null);
                setActionError(code as SessionErrorCode);
            }
        },
        [publishSession, queryClient],
    );

    const rejectWith = useCallback((error: unknown): never => {
        if (isLoginFailure(error) && CUSTODY_CODES.includes(error.code)) setActionError(error.code);
        throw error;
    }, []);

    const login = useCallback(
        async ({ origin, email, password, label }: SignInInput) => {
            setActionError(null);
            try {
                publishSession(await loginCommand.mutateAsync({ origin, email, password, label: label ?? null }));
            } catch (error) {
                rejectWith(error);
            }
        },
        [loginCommand, publishSession, rejectWith],
    );

    const register = useCallback(
        async ({ origin, displayName, email, password, label }: RegisterInput) => {
            setActionError(null);
            try {
                publishSession(await registerCommand.mutateAsync({ origin, displayName, email, password, label: label ?? null }));
            } catch (error) {
                rejectWith(error);
            }
        },
        [registerCommand, publishSession, rejectWith],
    );

    const logout = useCallback(
        async (summary?: ConnectionSummary) => {
            setActionError(null);
            try {
                await logoutCommand.mutateAsync(summary);
                publishSession(null);
            } catch (error) {
                const code = sessionErrorCode(error);
                if (["local_cleanup_failed", "cleanup_uncertain", "relogin_required"].includes(code)) {
                    publishSession(null);
                }
                setActionError(code);
                throw code;
            }
        },
        [logoutCommand, publishSession],
    );

    const retryCommit = useCallback(async () => {
        setActionError(null);
        try {
            publishSession(await retryCommitCommand.mutateAsync());
        } catch (error) {
            const code = sessionErrorCode(error);
            setActionError(code);
            throw code;
        }
    }, [retryCommitCommand, publishSession]);

    const retryRestore = useCallback(async () => {
        setActionError(null);
        const result = await restore.refetch();
        if (result.error) {
            const code = sessionErrorCode(result.error);
            setActionError(code);
            throw code;
        }
    }, [restore]);

    return (
        <AuthContext
            value={{
                session,
                reportInventoryCustodyError,
                issue,
                // A retried restore counts as loading, so the splash shows progress, not the old failure.
                isLoading: restore.isPending || restore.isRefetching,
                restoreError: restore.error ? sessionErrorCode(restore.error) : null,
                expired,
                login,
                register,
                logout,
                retryCommit,
                retryRestore,
                clearIssue: () => setActionError(null),
            }}
        >
            {children}
        </AuthContext>
    );
}

export function AuthProvider({ children }: { children: React.ReactNode }) {
    const [queryClient] = useState(() => new QueryClient());
    return (
        <QueryClientProvider client={queryClient}>
            <SessionAuthProvider>{children}</SessionAuthProvider>
        </QueryClientProvider>
    );
}
