import { relativeTime, shortDuration } from "@/lib/time";
import type { NodeSummary } from "@/types/nodes";

/** The card's role badge: LEADER for the leading manager. */
export function roleLabel(node: Pick<NodeSummary, "isLeader" | "role">): string {
    return node.isLeader ? "LEADER" : node.role.toUpperCase();
}

/** Why a node isn't active, in the card's note or banner; null when it is. */
export function healthNote(node: NodeSummary, now: number): string | null {
    const lastCheckIn = node.lastCheckInAt ? Date.parse(node.lastCheckInAt) : null;
    if (node.health === "down") {
        if (node.agentStatus === "offline" && lastCheckIn !== null) return `Node unreachable since ${relativeTime(lastCheckIn, now)}`;
        return "Swarm reports this node down";
    }
    if (node.health === "degraded") {
        if (node.agentStatus === "degraded" && lastCheckIn !== null) return `Agent last checked in ${shortDuration((now - lastCheckIn) / 1000)} ago`;
        if (node.agentStatus === "no-agent") return "No Kontainer agent has checked in from this node";
        return "Swarm state unknown: the manager didn't report this node";
    }
    return null;
}
