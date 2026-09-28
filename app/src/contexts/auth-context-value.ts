import { createContext } from "react";
import type { AuthState } from "@/types/auth";
import type { ConnectionProfile } from "@/types/connection";
import type { SessionErrorCode, SessionView } from "@/types/session";
import type { InventoryErrorCode } from "@/types/inventory";

export interface AuthContextValue extends AuthState {
    session: SessionView | null;
    issue: SessionErrorCode | null;
    profiles: ConnectionProfile[];
    activeProfileId: string | null;
    login: (origin: string, label?: string) => Promise<void>;
    logout: () => Promise<void>;
    retryCommit: () => Promise<void>;
    retryRestore: () => Promise<void>;
    cancelLogin: () => Promise<void>;
    clearIssue: () => void;
    reportInventoryCustodyError?: (profileId: string, code: InventoryErrorCode) => void;
}

export const AuthContext = createContext<AuthContextValue | null>(null);
