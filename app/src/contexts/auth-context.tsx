import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { AuthUser, ParsedToken } from "@/types/auth";
import type { ConnectionProfile, StoredProfiles } from "@/types/connection";
import { AuthContext } from "@/contexts/auth-context-value";

export function AuthProvider({ children }: { children: React.ReactNode }) {
    const [user, setUser] = useState<AuthUser | null>(null);
    const [token, setToken] = useState<string | null>(null);
    const [expiresAt, setExpiresAt] = useState<number | null>(null);
    const [agentUrl, setAgentUrl] = useState<string | null>(null);
    const [isLoading, setIsLoading] = useState(true);
    const [profiles, setProfiles] = useState<ConnectionProfile[]>([]);
    const [activeProfileId, setActiveProfileId] = useState<string | null>(null);

    const pendingLabelRef = useRef<string>("Default");

    const clearState = useCallback(() => {
        setUser(null);
        setToken(null);
        setExpiresAt(null);
        setAgentUrl(null);
    }, []);

    const applyProfile = useCallback((profile: ConnectionProfile) => {
        setUser({
            id: profile.userId,
            name: profile.userName,
            email: profile.userEmail,
            avatarUrl: profile.userAvatarUrl,
            provider: profile.userProvider,
        });
        setToken(profile.token);
        setExpiresAt(profile.expiresAt);
        setAgentUrl(profile.agentUrl);
        setActiveProfileId(profile.id);
    }, []);

    const logout = useCallback(async () => {
        try {
            await invoke("delete_session");
        } catch (err) {
            console.error("Failed to delete session:", err);
        }
        clearState();

        // Reload profiles after removing the active one
        try {
            const data = await invoke<StoredProfiles>("load_profiles");
            setProfiles(data.profiles);
            setActiveProfileId(data.activeProfileId);
        } catch {
            setProfiles([]);
            setActiveProfileId(null);
        }
    }, [clearState]);

    const handleToken = useCallback(async (jwt: string, loginAgentUrl: string) => {
        try {
            const parsed = await invoke<ParsedToken>("parse_auth_token", {
                token: jwt,
            });

            const profileId = crypto.randomUUID();
            const label = pendingLabelRef.current;

            setUser(parsed.user);
            setToken(jwt);
            setExpiresAt(parsed.expiresAt);
            setAgentUrl(loginAgentUrl);

            try {
                await invoke("save_profile", {
                    input: {
                        id: profileId,
                        label,
                        agentUrl: loginAgentUrl,
                        userId: parsed.user.id,
                        userName: parsed.user.name,
                        userEmail: parsed.user.email,
                        userAvatarUrl: parsed.user.avatarUrl,
                        userProvider: parsed.user.provider,
                        token: jwt,
                        expiresAt: parsed.expiresAt,
                    },
                });
            } catch (err) {
                console.error("Failed to save profile:", err);
            }

            // Reload profiles
            try {
                const data = await invoke<StoredProfiles>("load_profiles");
                setProfiles(data.profiles);
                setActiveProfileId(data.activeProfileId);
            } catch {
                // At minimum set the new profile locally
                setActiveProfileId(profileId);
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
                const data = await invoke<StoredProfiles>("load_profiles");
                if (cancelled) return;

                setProfiles(data.profiles);
                setActiveProfileId(data.activeProfileId);

                if (data.activeProfileId) {
                    const active = data.profiles.find((p) => p.id === data.activeProfileId);
                    if (active) {
                        const now = Math.floor(Date.now() / 1000);
                        if (active.expiresAt <= now) {
                            await invoke("delete_profile", { profileId: active.id });
                            const refreshed = await invoke<StoredProfiles>("load_profiles");
                            if (!cancelled) {
                                setProfiles(refreshed.profiles);
                                setActiveProfileId(refreshed.activeProfileId);
                            }
                            return;
                        }

                        setUser({
                            id: active.userId,
                            name: active.userName,
                            email: active.userEmail,
                            avatarUrl: active.userAvatarUrl,
                            provider: active.userProvider,
                        });
                        setToken(active.token);
                        setExpiresAt(active.expiresAt);
                        setAgentUrl(active.agentUrl);
                    }
                }
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

    // Expiry timer
    useEffect(() => {
        const interval = setInterval(async () => {
            if (expiresAt !== null && expiresAt <= Math.floor(Date.now() / 1000)) {
                if (activeProfileId) {
                    try {
                        await invoke("delete_profile", { profileId: activeProfileId });
                    } catch {
                        // ignore
                    }
                }
                clearState();

                // Reload profiles, switch to another if available
                try {
                    const data = await invoke<StoredProfiles>("load_profiles");
                    setProfiles(data.profiles);
                    setActiveProfileId(data.activeProfileId);

                    if (data.profiles.length > 0) {
                        const next = data.profiles[0];
                        await invoke("set_active_profile", { profileId: next.id });
                        applyProfile(next);
                    } else {
                        setActiveProfileId(null);
                    }
                } catch {
                    setProfiles([]);
                    setActiveProfileId(null);
                }
            }
        }, 30_000);

        return () => clearInterval(interval);
    }, [expiresAt, activeProfileId, clearState, applyProfile]);

    const login = useCallback(
        async (url: string, label?: string, provider?: string) => {
            pendingLabelRef.current = label || "Default";
            const authProvider = provider || "github";

            // Set up all event listeners BEFORE starting the WebSocket
            const unlistenSessionId = await listen<{ sessionId: string }>("auth://session-id", (event) => {
                openUrl(`${url}/auth/${authProvider}?session=${event.payload.sessionId}`);
            });

            let resolveToken: (jwt: string) => void;
            let rejectToken: (err: Error) => void;
            const tokenPromise = new Promise<string>((resolve, reject) => {
                resolveToken = resolve;
                rejectToken = reject;
            });

            const unlistenToken = await listen<{ token: string }>("auth://token", (event) => {
                resolveToken(event.payload.token);
            });

            const unlistenError = await listen<{ error: string }>("auth://error", (event) => {
                rejectToken(new Error(event.payload.error));
            });

            try {
                // Fire-and-forget: starts the background WebSocket
                await invoke("auth_listen", { url });

                // Wait for token or error event from the background task
                const jwt = await tokenPromise;
                await handleToken(jwt, url);
            } finally {
                unlistenSessionId();
                unlistenToken();
                unlistenError();
            }
        },
        [handleToken],
    );

    const switchProfile = useCallback(
        async (profileId: string) => {
            try {
                await invoke("set_active_profile", { profileId });
                const data = await invoke<StoredProfiles>("load_profiles");
                setProfiles(data.profiles);
                setActiveProfileId(data.activeProfileId);

                const profile = data.profiles.find((p) => p.id === profileId);
                if (profile) {
                    applyProfile(profile);
                }
            } catch (err) {
                console.error("Failed to switch profile:", err);
            }
        },
        [applyProfile],
    );

    const removeProfile = useCallback(async (profileId: string) => {
        try {
            await invoke("delete_profile", { profileId });
            const data = await invoke<StoredProfiles>("load_profiles");
            setProfiles(data.profiles);
            setActiveProfileId(data.activeProfileId);
        } catch (err) {
            console.error("Failed to remove profile:", err);
        }
    }, []);

    return (
        <AuthContext
            value={{
                user,
                token,
                expiresAt,
                agentUrl,
                isLoading,
                profiles,
                activeProfileId,
                login,
                logout,
                switchProfile,
                removeProfile,
            }}
        >
            {children}
        </AuthContext>
    );
}
