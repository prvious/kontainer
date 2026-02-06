import { createContext } from "react";
import type { AuthState } from "@/types/auth";

export interface AuthContextValue extends AuthState {
    login: (agentUrl: string) => Promise<void>;
    logout: () => void;
}

export const AuthContext = createContext<AuthContextValue | null>(null);
