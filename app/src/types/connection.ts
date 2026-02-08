export interface ConnectionProfile {
    id: string;
    label: string;
    agentUrl: string;
    userId: string;
    userName: string;
    userEmail: string;
    userAvatarUrl: string;
    userProvider: string;
    token: string;
    expiresAt: number;
}

export interface StoredProfiles {
    profiles: ConnectionProfile[];
    activeProfileId: string | null;
}
