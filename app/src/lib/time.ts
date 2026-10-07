/** Compact relative time for status lines: "just now", "5m ago", "3h ago", "2d ago". */
export function relativeTime(thenMs: number, nowMs: number): string {
    const seconds = Math.max(0, Math.round((nowMs - thenMs) / 1000));
    if (seconds < 45) return "just now";
    const minutes = Math.round(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.round(minutes / 60);
    if (hours < 48) return `${hours}h ago`;
    return `${Math.round(hours / 24)}d ago`;
}

/** 24-hour wall-clock time, e.g. "14:32" or with seconds "14:32:05". */
export function clockTime(ms: number, withSeconds = false): string {
    return new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: withSeconds ? "2-digit" : undefined, hour12: false });
}

/** "4s", "3m", "1h 5m": the shortest honest duration. */
export function shortDuration(totalSeconds: number): string {
    const seconds = Math.max(0, Math.round(totalSeconds));
    if (seconds < 60) return `${seconds}s`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    return minutes % 60 === 0 ? `${hours}h` : `${hours}h ${minutes % 60}m`;
}

/** `HH:mm:ss.SSS` in local time, for log lines. */
export function logTime(ms: number): string {
    const date = new Date(ms);
    const pad = (value: number, width = 2) => String(value).padStart(width, "0");
    return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}.${pad(date.getMilliseconds(), 3)}`;
}
