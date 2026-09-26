export interface AuthUser {
    id: string;
    name: string;
    email: string;
    avatarUrl: string;
    provider: string;
}

export interface ParsedToken {
    userId: string;
    expiresAt: number;
}

export interface CheckAuthResponse {
    user: AuthUser;
    status: string;
}

export interface AuthState {
    user: AuthUser | null;
    token: null;
    expiresAt: number | null;
    agentUrl: string | null;
    isLoading: boolean;
}
