export function formatRelativeTime(iso: string): string {
    const seconds = Math.floor((Date.now() - new Date(iso).getTime()) / 1000);
    if (seconds < 60) return "just now";
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
}

export function getStateColor(state: string): string {
    switch (state.toLowerCase()) {
        case "running":
            return "bg-success";
        case "failed":
        case "rejected":
            return "bg-destructive";
        case "complete":
        case "shutdown":
            return "bg-muted-foreground";
        case "preparing":
        case "pending":
        case "assigned":
        case "accepted":
            return "bg-warning";
        default:
            return "bg-muted-foreground";
    }
}
