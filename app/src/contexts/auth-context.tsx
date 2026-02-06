import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { AuthUser, ParsedToken, StoredSession } from "@/types/auth";
import { AuthContext } from "@/contexts/auth-context-value";

export function AuthProvider({ children }: { children: React.ReactNode }) {
    const [user, setUser] = useState<AuthUser | null>(null);
    const [token, setToken] = useState<string | null>(null);
    const [expiresAt, setExpiresAt] = useState<number | null>(null);
    const [agentUrl, setAgentUrl] = useState<string | null>(null);
    const [isLoading, setIsLoading] = useState(true);

    const pendingAgentUrl = useRef<string | null>(null);

    const clearState = useCallback(() => {
        setUser(null);
        setToken(null);
        setExpiresAt(null);
        setAgentUrl(null);
    }, []);

    const logout = useCallback(async () => {
        try {
            await invoke("delete_session");
        } catch (err) {
            console.error("Failed to delete session:", err);
        }
        clearState();
    }, [clearState]);

    const handleToken = useCallback(async (jwt: string) => {
        try {
            const parsed = await invoke<ParsedToken>("parse_auth_token", {
                token: jwt,
            });
            setUser(parsed.user);
            setToken(jwt);
            setExpiresAt(parsed.expiresAt);

            const savedAgentUrl = pendingAgentUrl.current || null;
            setAgentUrl(savedAgentUrl);

            try {
                await invoke("save_session", {
                    input: {
                        userId: parsed.user.id,
                        userName: parsed.user.name,
                        userEmail: parsed.user.email,
                        userAvatarUrl: parsed.user.avatarUrl,
                        userProvider: parsed.user.provider,
                        token: jwt,
                        expiresAt: parsed.expiresAt,
                        agentUrl: savedAgentUrl || "",
                    },
                });
            } catch (err) {
                console.error("Failed to save session:", err);
            }
        } catch (err) {
            console.error("Failed to parse auth token:", err);
        }
    }, []);

    // Restore session on mount
    useEffect(() => {
        let cancelled = false;

        async function restore() {
            try {
                const session = await invoke<StoredSession | null>("load_session");
                if (cancelled || !session) return;

                const now = Math.floor(Date.now() / 1000);
                if (session.expiresAt <= now) {
                    await invoke("delete_session");
                    return;
                }

                setUser({
                    id: session.userId,
                    name: session.userName,
                    email: session.userEmail,
                    avatarUrl: session.userAvatarUrl,
                    provider: session.userProvider,
                });
                setToken(session.token);
                setExpiresAt(session.expiresAt);
                setAgentUrl(session.agentUrl || null);
            } catch (err) {
                console.error("Failed to restore session:", err);
            } finally {
                if (!cancelled) {
                    setIsLoading(false);
                }
            }
        }

        restore();

        return () => {
            cancelled = true;
        };
    }, []);

    // Deep link listener
    useEffect(() => {
        const unlisten = onOpenUrl((urls) => {
            for (const raw of urls) {
                const url = new URL(raw);
                if (url.host === "auth" && url.pathname === "/callback") {
                    const jwt = url.searchParams.get("token");
                    if (jwt) {
                        handleToken(jwt);
                    }
                }
            }
        });

        return () => {
            unlisten.then((fn) => fn());
        };
    }, [handleToken]);

    // Expiry timer
    useEffect(() => {
        const interval = setInterval(() => {
            if (expiresAt !== null && expiresAt <= Math.floor(Date.now() / 1000)) {
                logout();
            }
        }, 30_000);

        return () => clearInterval(interval);
    }, [expiresAt, logout]);

    const login = useCallback(async (url: string) => {
        pendingAgentUrl.current = url;
        await openUrl(`${url}/auth/github`);
    }, []);

    return <AuthContext value={{ user, token, expiresAt, agentUrl, isLoading, login, logout }}>{children}</AuthContext>;
}
