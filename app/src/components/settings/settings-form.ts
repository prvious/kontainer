import { originError } from "@/components/connect/connect-rules";
import { hasControlCharacter } from "@/types/inventory";
import { DEFAULT_APP_SETTINGS, DEFAULT_TERMINAL, colorSchemeFrom, type AppSettings, type ColorScheme } from "@/types/settings";

/**
 * The Settings page's staged form (08.S5): a draft of every editable row, what changed against
 * the stored values, and why a field can't be saved. Nothing here writes anything.
 */

export const THEMES: { value: ColorScheme; label: string }[] = [
    { value: "system", label: "System" },
    { value: "light", label: "Light" },
    { value: "dark", label: "Dark" },
];
export const REFRESH_INTERVALS = [2_000, 5_000, 10_000, 30_000, 60_000];
export const SHELLS = ["auto", "/bin/bash", "/bin/sh", "/bin/zsh", "custom"] as const;
export const SCROLLBACKS = [1_000, 5_000, 10_000, 50_000];
export const RETENTION_DAYS = [1, 3, 7, 14, 30];
export const FONT_SIZE_MIN = 10;
export const FONT_SIZE_MAX = 24;
const MAX_SHELL_LENGTH = 256;

export type ShellChoice = (typeof SHELLS)[number];

export interface SettingsDraft {
    scheme: ColorScheme;
    refreshInterval: number;
    shell: ShellChoice;
    /** The path typed when `shell` is `custom`. */
    customShell: string;
    /** As typed, so an out-of-range size can be shown with its reason. */
    fontSize: string;
    scrollback: number;
    agentUrl: string;
    /** Null until the controller's settings have loaded. */
    retentionDays: number | null;
}

/** The rows the page stages, in page order. */
export const ROWS = ["scheme", "refreshInterval", "shell", "fontSize", "scrollback", "agentUrl", "retentionDays"] as const;
export type SettingsRow = (typeof ROWS)[number];

export const ROW_LABELS: Record<SettingsRow, string> = {
    scheme: "Theme",
    refreshInterval: "Dashboard refresh interval",
    shell: "Default terminal shell",
    fontSize: "Terminal font size",
    scrollback: "Terminal scrollback buffer",
    agentUrl: "Agent URL",
    retentionDays: "Metrics retention",
};

export function draftFrom(settings: AppSettings, origin: string, retentionDays: number | null): SettingsDraft {
    const stored = settings.terminalShell?.trim() ?? "";
    const preset = SHELLS.find((shell) => shell !== "auto" && shell !== "custom" && shell === stored);
    return {
        scheme: colorSchemeFrom(settings.scheme),
        refreshInterval: settings.refreshInterval ?? DEFAULT_APP_SETTINGS.refreshInterval ?? 5_000,
        shell: stored === "" ? "auto" : (preset ?? "custom"),
        customShell: preset || stored === "" ? "" : stored,
        fontSize: String(settings.terminalFontSize ?? DEFAULT_TERMINAL.fontSize),
        scrollback: settings.terminalScrollback ?? DEFAULT_TERMINAL.scrollback,
        agentUrl: origin,
        retentionDays,
    };
}

/** The shell exec sessions open with; null lets the node pick (bash, then sh). */
export function shellOf(draft: SettingsDraft): string | null {
    if (draft.shell === "auto") return null;
    return draft.shell === "custom" ? draft.customShell.trim() : draft.shell;
}

/** Two spellings of one controller (`…/` and `…`) are the same URL. */
function sameUrl(left: string, right: string): boolean {
    try {
        return new URL(left.trim()).origin === new URL(right.trim()).origin;
    } catch {
        return left.trim() === right.trim();
    }
}

function rowChanged(row: SettingsRow, baseline: SettingsDraft, draft: SettingsDraft): boolean {
    switch (row) {
        case "shell":
            return shellOf(baseline) !== shellOf(draft);
        case "fontSize":
            return Number(baseline.fontSize) !== Number(draft.fontSize);
        case "agentUrl":
            return !sameUrl(baseline.agentUrl, draft.agentUrl);
        default:
            return baseline[row] !== draft[row];
    }
}

export function changedRows(baseline: SettingsDraft, draft: SettingsDraft): SettingsRow[] {
    return ROWS.filter((row) => rowChanged(row, baseline, draft));
}

/** Why each invalid field can't be saved. */
export function fieldErrors(draft: SettingsDraft): Partial<Record<SettingsRow, string>> {
    const errors: Partial<Record<SettingsRow, string>> = {};
    const url = originError(draft.agentUrl);
    if (url) errors.agentUrl = url;
    if (draft.shell === "custom") {
        const shell = draft.customShell.trim();
        if (!shell.startsWith("/") || shell.length > MAX_SHELL_LENGTH || /\s/.test(shell) || hasControlCharacter(shell))
            errors.shell = "Use an absolute path without spaces, such as /bin/ash.";
    }
    const size = Number(draft.fontSize);
    if (!/^\d+$/.test(draft.fontSize.trim()) || size < FONT_SIZE_MIN || size > FONT_SIZE_MAX)
        errors.fontSize = `Use a size from ${FONT_SIZE_MIN} to ${FONT_SIZE_MAX}.`;
    return errors;
}

export const seconds = (milliseconds: number) => `${milliseconds / 1_000}s`;
export const days = (count: number) => `${count} ${count === 1 ? "day" : "days"}`;

/** A row's value as the `changed from …` note and the saved toast say it. */
export function valueText(row: SettingsRow, draft: SettingsDraft): string {
    switch (row) {
        case "scheme":
            return THEMES.find((theme) => theme.value === draft.scheme)?.label ?? draft.scheme;
        case "refreshInterval":
            return seconds(draft.refreshInterval);
        case "shell":
            return shellOf(draft) ?? "auto";
        case "fontSize":
            return draft.fontSize.trim();
        case "scrollback":
            return draft.scrollback.toLocaleString("en-US");
        case "agentUrl":
            return draft.agentUrl.trim();
        case "retentionDays":
            return draft.retentionDays === null ? "—" : days(draft.retentionDays);
    }
}

/** The stored settings with the draft's application rows applied; other keys are kept. */
export function appSettingsFrom(settings: AppSettings, draft: SettingsDraft): AppSettings {
    return {
        ...settings,
        scheme: draft.scheme,
        refreshInterval: draft.refreshInterval,
        terminalShell: shellOf(draft),
        terminalFontSize: Number(draft.fontSize),
        terminalScrollback: draft.scrollback,
    };
}

/** Choices for a select, keeping a stored value the list doesn't offer so it still shows. */
export function withCurrent(choices: number[], current: number | null): number[] {
    return current === null || choices.includes(current) ? choices : [...choices, current].sort((left, right) => left - right);
}
