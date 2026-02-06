export interface AuthUser {
    id: string;
    name: string;
    email: string;
    avatarUrl: string;
    provider: string;
}

export interface ParsedToken {
    user: AuthUser;
    expiresAt: number;
}

export interface AuthState {
    user: AuthUser | null;
    token: string | null;
    expiresAt: number | null;
}
