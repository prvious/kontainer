import { relativeTime, shortDuration } from "@/lib/time";
import type { NodeSummary } from "@/types/nodes";

/** The card's role badge: LEADER for the leading manager. */
export function roleLabel(node: Pick<NodeSummary, "isLeader" | "role">): string {
    return node.isLeader ? "LEADER" : node.role.toUpperCase();
}

/** The controller's online window: an agent that checked in this recently is up. */
const ONLINE_WITHIN_MS = 15_000;

/** Why a node isn't active, in the card's note or banner; null when it is. */
export function healthNote(node: NodeSummary, now: number): string | null {
    const lastCheckIn = node.lastCheckInAt ? Date.parse(node.lastCheckInAt) : null;
    if (node.health === "down") {
        if (node.agentStatus === "offline" && lastCheckIn !== null) return `Node unreachable since ${relativeTime(lastCheckIn, now)}`;
        return "Swarm reports this node down";
    }
    if (node.health === "degraded") {
        // An agent that keeps checking in but sends no fresh sample can't read its Docker.
        if (node.agentStatus === "degraded" && lastCheckIn !== null && node.latestSample === null && now - lastCheckIn <= ONLINE_WITHIN_MS) {
            return "The agent can't read Docker on this node";
        }
        if (node.agentStatus === "degraded" && lastCheckIn !== null) return `Agent last checked in ${shortDuration((now - lastCheckIn) / 1000)} ago`;
        if (node.agentStatus === "no-agent") return "No Kontainer agent has checked in from this node";
        return "Swarm state unknown: the manager didn't report this node";
    }
    return null;
}
