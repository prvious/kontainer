import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { AuthUser, ParsedToken } from "@/types/auth";
import { AuthContext } from "@/contexts/auth-context-value";

export function AuthProvider({ children }: { children: React.ReactNode }) {
    const [user, setUser] = useState<AuthUser | null>(null);
    const [token, setToken] = useState<string | null>(null);
    const [expiresAt, setExpiresAt] = useState<number | null>(null);

    const handleToken = useCallback(async (jwt: string) => {
        try {
            const parsed = await invoke<ParsedToken>("parse_auth_token", {
                token: jwt,
            });
            setUser(parsed.user);
            setToken(jwt);
            setExpiresAt(parsed.expiresAt);
        } catch (err) {
            console.error("Failed to parse auth token:", err);
        }
    }, []);

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

    const login = useCallback(async (agentUrl: string) => {
        await openUrl(`${agentUrl}/auth/github`);
    }, []);

    const logout = useCallback(() => {
        setUser(null);
        setToken(null);
        setExpiresAt(null);
    }, []);

    return <AuthContext value={{ user, token, expiresAt, login, logout }}>{children}</AuthContext>;
}
