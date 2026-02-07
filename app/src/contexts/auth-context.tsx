import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { AuthUser, ParsedToken, StoredSession } from "@/types/auth";
import { AuthContext } from "@/contexts/auth-context-value";

export function AuthProvider({ children }: { children: React.ReactNode }) {
    const [user, setUser] = useState<AuthUser | null>(null);
    const [token, setToken] = useState<string | null>(null);
    const [expiresAt, setExpiresAt] = useState<number | null>(null);
    const [agentUrl, setAgentUrl] = useState<string | null>(null);
    const [isLoading, setIsLoading] = useState(true);

    const wsRef = useRef<WebSocket | null>(null);

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

    const handleToken = useCallback(async (jwt: string, loginAgentUrl: string) => {
        try {
            const parsed = await invoke<ParsedToken>("parse_auth_token", {
                token: jwt,
            });
            setUser(parsed.user);
            setToken(jwt);
            setExpiresAt(parsed.expiresAt);
            setAgentUrl(loginAgentUrl);

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
                        agentUrl: loginAgentUrl,
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

    // Cleanup WS on unmount
    useEffect(() => {
        return () => {
            wsRef.current?.close();
        };
    }, []);

    // Expiry timer
    useEffect(() => {
        const interval = setInterval(() => {
            if (expiresAt !== null && expiresAt <= Math.floor(Date.now() / 1000)) {
                logout();
            }
        }, 30_000);

        return () => clearInterval(interval);
    }, [expiresAt, logout]);

    const login = useCallback(
        (url: string) => {
            return new Promise<void>((resolve, reject) => {
                const wsUrl = url.replace(/^http/, "ws") + "/auth/listen";
                const ws = new WebSocket(wsUrl);
                wsRef.current = ws;
                let settled = false;

                const timeout = setTimeout(
                    () => {
                        settled = true;
                        ws.close();
                        reject(new Error("Authentication timed out"));
                    },
                    10 * 60 * 1000,
                );

                ws.onmessage = (ev) => {
                    const data = JSON.parse(ev.data);

                    if (data.sessionId) {
                        openUrl(`${url}/auth/github?session=${data.sessionId}`);
                        return;
                    }

                    if (data.token) {
                        settled = true;
                        clearTimeout(timeout);
                        ws.close();
                        handleToken(data.token, url).then(resolve, reject);
                        return;
                    }

                    if (data.error) {
                        settled = true;
                        clearTimeout(timeout);
                        ws.close();
                        reject(new Error(data.error));
                    }
                };

                ws.onerror = () => {
                    if (settled) return;
                    settled = true;
                    clearTimeout(timeout);
                    reject(new Error("WebSocket connection failed"));
                };

                ws.onclose = (ev) => {
                    if (settled) return;
                    settled = true;
                    clearTimeout(timeout);
                    if (!ev.wasClean) {
                        reject(new Error("WebSocket closed unexpectedly"));
                    }
                };
            });
        },
        [handleToken],
    );

    return <AuthContext value={{ user, token, expiresAt, agentUrl, isLoading, login, logout }}>{children}</AuthContext>;
}
