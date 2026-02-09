export interface AppSettings {
    refreshInterval: number | null;
}

export interface AgentSettings {
    settings: Record<string, string>;
}
