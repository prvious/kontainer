export interface AppSettings {
    refreshInterval: number | null;
    terminalShell: string | null;
    terminalFontSize: number | null;
    terminalFontFamily: string | null;
    terminalScrollback: number | null;
}

export interface AgentSettings {
    settings: Record<string, string>;
}
