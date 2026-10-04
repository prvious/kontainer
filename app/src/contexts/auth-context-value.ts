import { createContext } from "react";
import type { ConnectionSummary, SessionErrorCode, SessionView } from "@/types/session";
import type { InventoryErrorCode } from "@/types/inventory";

export interface SignInInput {
    origin: string;
    email: string;
    password: string;
    label?: string | null;
}

export interface RegisterInput extends SignInInput {
    displayName: string;
    /** The one-time code the controller prints at startup while it has no owner. */
    setupCode: string;
}

export interface AuthContextValue {
    session: SessionView | null;
    isLoading: boolean;
    issue: SessionErrorCode | null;
    /** The code session restore failed with, if it did; the splash reports startup failures. */
    restoreError: SessionErrorCode | null;
    /**
     * The controller rejected the session (state 02.S5). The last session view stays so the
     * page remains mounted under the dialog; data hooks pause while this is true.
     */
    expired: boolean;
    /** Rejects with a `LoginFailure`. */
    login: (input: SignInInput) => Promise<void>;
    /** Rejects with a `LoginFailure`. */
    register: (input: RegisterInput) => Promise<void>;
    /** `nextOrigin` is the agent Connect opens with once signed out (switching agents). */
    logout: (summary?: ConnectionSummary, nextOrigin?: string) => Promise<void>;
    /** Set by a sign-out that switches agents; the signed-out redirect fills Connect with it. */
    nextOrigin?: string | null;
    retryCommit: () => Promise<void>;
    retryRestore: () => Promise<void>;
    clearIssue: () => void;
    reportInventoryCustodyError?: (profileId: string, code: InventoryErrorCode) => void;
}

export const AuthContext = createContext<AuthContextValue | null>(null);
