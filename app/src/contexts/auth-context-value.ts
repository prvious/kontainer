import { createContext } from "react";
import type { AuthState } from "@/types/auth";
import type { ConnectionProfile } from "@/types/connection";

export interface AuthContextValue extends AuthState {
    profiles: ConnectionProfile[];
    activeProfileId: string | null;
    login: (agentUrl: string, label?: string, provider?: string) => Promise<void>;
    logout: () => Promise<void>;
    switchProfile: (profileId: string) => Promise<void>;
    removeProfile: (profileId: string) => Promise<void>;
    updateProfile: (profileId: string, updates: Partial<Pick<ConnectionProfile, "label" | "agentUrl">>) => Promise<void>;
}

export const AuthContext = createContext<AuthContextValue | null>(null);
