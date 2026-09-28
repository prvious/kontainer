import { useCallback, useState } from "react";
import { QueryClient, QueryClientProvider, useQueryClient } from "@tanstack/react-query";
import { AuthContext } from "@/contexts/auth-context-value";
import { useSessionCancelLogin } from "@/hooks/session/use-session-cancel-login";
import { useSessionLogin } from "@/hooks/session/use-session-login";
import { useSessionLogout } from "@/hooks/session/use-session-logout";
import { useSessionRestore, sessionRestoreKey } from "@/hooks/session/use-session-restore";
import { useSessionRetryCommit } from "@/hooks/session/use-session-retry-commit";
import { sessionErrorCode, type SessionErrorCode, type SessionView } from "@/types/session";
import type { InventoryErrorCode } from "@/types/inventory";

function SessionAuthProvider({ children }: { children: React.ReactNode }) {
    const queryClient = useQueryClient();
    const restore = useSessionRestore();
    const loginCommand = useSessionLogin();
    const logoutCommand = useSessionLogout();
    const retryCommitCommand = useSessionRetryCommit();
    const cancelLoginCommand = useSessionCancelLogin();
    const [actionError, setActionError] = useState<SessionErrorCode | null>(null);

    const session = restore.data ?? null;
    const issue = actionError ?? (restore.error ? sessionErrorCode(restore.error) : null);

    const publishSession = useCallback(
        (view: SessionView | null) => {
            // Controller data belongs to the signed-in profile: drop it on
            // replacement or logout. App-level data (["app", ...]) stays.
            queryClient.removeQueries({ predicate: (query) => query.queryKey[0] !== "session" && query.queryKey[0] !== "app" });
            queryClient.getMutationCache().clear();
            queryClient.setQueryData(sessionRestoreKey, view);
            queryClient.removeQueries({ queryKey: ["session", "login-hint"] });
            if (view) {
                queryClient.setQueryData(["session", "login-hint"], { origin: view.origin, label: view.label });
            }
        },
        [queryClient],
    );

    const reportInventoryCustodyError = useCallback(
        (profileId: string, code: InventoryErrorCode) => {
            const current = queryClient.getQueryData<SessionView | null>(sessionRestoreKey);
            if (current?.profileId !== profileId) return;
            const sessionCode: SessionErrorCode | null =
                code === "unauthenticated"
                    ? "relogin_required"
                    : ["relogin_required", "vault_unavailable", "vault_write_failed", "local_state_failed", "cleanup_uncertain"].includes(code)
                      ? (code as SessionErrorCode)
                      : null;
            if (sessionCode) {
                publishSession(null);
                setActionError(sessionCode);
            }
        },
        [publishSession, queryClient],
    );

    const login = useCallback(
        async (origin: string, label?: string) => {
            setActionError(null);
            try {
                const view = await loginCommand.mutateAsync({ origin, label: label ?? null });
                publishSession(view);
            } catch (error) {
                const code = sessionErrorCode(error);
                setActionError(code);
                throw code;
            }
        },
        [loginCommand, publishSession],
    );

    const logout = useCallback(async () => {
        setActionError(null);
        try {
            await logoutCommand.mutateAsync();
            publishSession(null);
        } catch (error) {
            const code = sessionErrorCode(error);
            if (["local_cleanup_failed", "cleanup_uncertain", "relogin_required"].includes(code)) {
                publishSession(null);
            }
            setActionError(code);
            throw code;
        }
    }, [logoutCommand, publishSession]);

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

    const cancelLogin = useCallback(async () => {
        try {
            await cancelLoginCommand.mutateAsync();
        } catch (error) {
            const code = sessionErrorCode(error);
            setActionError(code);
            throw code;
        }
    }, [cancelLoginCommand]);

    return (
        <AuthContext
            value={{
                session,
                reportInventoryCustodyError,
                issue,
                // A retried restore counts as loading, so the splash shows progress, not the old failure.
                isLoading: restore.isPending || restore.isRefetching,
                restoreError: restore.error ? sessionErrorCode(restore.error) : null,
                login,
                logout,
                retryCommit,
                retryRestore,
                cancelLogin,
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
