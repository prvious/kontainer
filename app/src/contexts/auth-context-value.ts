import { createContext } from "react";
import type { SessionErrorCode, SessionView } from "@/types/session";
import type { InventoryErrorCode } from "@/types/inventory";

export interface AuthContextValue {
    session: SessionView | null;
    isLoading: boolean;
    issue: SessionErrorCode | null;
    login: (origin: string, label?: string) => Promise<void>;
    logout: () => Promise<void>;
    retryCommit: () => Promise<void>;
    retryRestore: () => Promise<void>;
    cancelLogin: () => Promise<void>;
    clearIssue: () => void;
    reportInventoryCustodyError?: (profileId: string, code: InventoryErrorCode) => void;
}

export const AuthContext = createContext<AuthContextValue | null>(null);
