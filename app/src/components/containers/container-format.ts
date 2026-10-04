import type { Tone } from "@/components/kit/badges";
import { relativeTime } from "@/lib/time";
import type { ContainerRow } from "@/hooks/use-containers";

/** StatusChip label and tone per state; a stale row is UNKNOWN. */
export function statusChip(row: Pick<ContainerRow, "state" | "stale">): { label: string; tone: Tone } {
    if (row.stale) return { label: "UNKNOWN", tone: "neutral" };
    switch (row.state) {
        case "running":
            return { label: "UP", tone: "success" };
        case "paused":
            return { label: "PAUSED", tone: "warning" };
        case "exited":
            return { label: "EXITED", tone: "destructive" };
        case "dead":
            return { label: "DEAD", tone: "destructive" };
        case "restarting":
            return { label: "RESTARTING", tone: "warning" };
        case "created":
            return { label: "CREATED", tone: "neutral" };
        case "removing":
            return { label: "REMOVING", tone: "neutral" };
        case "stopping":
            return { label: "STOPPING", tone: "warning" };
        default:
            return { label: "UNKNOWN", tone: "neutral" };
    }
}

/** `14d 7h`, `3h 12m`, `8m`, `42s`. */
export function uptime(seconds: number): string {
    const total = Math.max(0, Math.floor(seconds));
    const days = Math.floor(total / 86_400);
    const hours = Math.floor((total % 86_400) / 3_600);
    const minutes = Math.floor((total % 3_600) / 60);
    if (days > 0) return `${days}d ${hours}h`;
    if (hours > 0) return `${hours}h ${minutes}m`;
    if (minutes > 0) return `${minutes}m`;
    return `${total}s`;
}

/** UPTIME cell: running for, `exited 2m ago`, `restarting`, or `last seen 2h ago` for stale rows. */
export function uptimeText(row: ContainerRow, now: number): string {
    if (row.stale) return `last seen ${relativeTime(Date.parse(row.lastSeenAt), now)}`;
    if (row.state === "restarting") return "restarting";
    if ((row.state === "running" || row.state === "paused") && row.startedAt) return uptime((now - Date.parse(row.startedAt)) / 1000);
    if ((row.state === "exited" || row.state === "dead") && row.finishedAt) return `exited ${relativeTime(Date.parse(row.finishedAt), now)}`;
    return "—";
}

/** `128 MB`, `2.1 GB`. */
export function bytes(value: number | null): string {
    if (value === null) return "—";
    const units = ["B", "KB", "MB", "GB", "TB"];
    let size = value;
    let unit = 0;
    while (size >= 1024 && unit < units.length - 1) {
        size /= 1024;
        unit += 1;
    }
    const digits = size >= 100 || unit < 2 ? 0 : 1;
    return `${size.toFixed(digits).replace(/\.0$/, "")} ${units[unit]}`;
}

/** Host ports, e.g. `80, 443`; `—` when nothing is published. */
export function portsText(ports: string[]): string {
    if (ports.length === 0) return "—";
    return [...new Set(ports.map((port) => port.split(":")[0]))].join(", ");
}

export const isRunning = (row: Pick<ContainerRow, "state">) => row.state === "running";

/** Running, paused, or restarting: Docker removes it only with `force`, which kills it first. */
export const needsForce = (row: Pick<ContainerRow, "state">) => row.state === "running" || row.state === "paused" || row.state === "restarting";
