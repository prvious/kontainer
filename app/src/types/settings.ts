export interface AppSettings {
    refreshInterval: number | null;
    terminalShell: string | null;
    terminalFontSize: number | null;
    terminalFontFamily: string | null;
    terminalScrollback: number | null;
    font: string | null;
}

export interface AgentSettings {
    settings: Record<string, string>;
}
