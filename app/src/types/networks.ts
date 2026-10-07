/**
 * Networks crossing the Tauri boundary.
 *
 * Mirrors `app/src-tauri/src/session/manager_networks.rs`; the shared fixture at
 * `app/tests/fixtures/networks-ipc.json` keeps both sides in sync.
 */
import type { NodeOutcomeView } from "@/types/containers";

export const NETWORK_SCOPES = ["swarm", "local"] as const;
export type NetworkScope = (typeof NETWORK_SCOPES)[number];

export const MANAGER_STATUSES = ["ok", "not_a_manager", "unavailable"] as const;
export type ManagerStatus = (typeof MANAGER_STATUSES)[number];

export interface NetworkView {
    id: string;
    name: string;
    driver: string;
    /** `swarm`: one row for the cluster. `local`: one row per node. */
    scope: NetworkScope;
    subnets: string[];
    /** Running containers attached, summed across nodes. */
    containers: number;
    createdAt: string | null;
    attachable: boolean;
    /** Docker's own networks, which can never be removed. */
    builtin: boolean;
    /** Local: its node. Swarm: the nodes with endpoints on it. */
    nodeIds: string[];
}

export interface NetworkListView {
    items: NetworkView[];
    nodes: NodeOutcomeView[];
    /** Whether the swarm-scope rows came from the Swarm manager. */
    manager: ManagerStatus;
    snapshotAt: string;
}

export interface AttachedContainerView {
    nodeId: string;
    containerId: string;
    name: string;
}

/** Names the controller creates: `[a-zA-Z0-9][a-zA-Z0-9_.-]*`, 1–128 characters. */
export const NETWORK_NAME = /^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$/;
export const BUILTIN_NETWORKS = ["host", "bridge", "none", "docker_gwbridge", "ingress"];

/** Stable row id: a swarm network is one row; a local one is unique per node. */
export const networkRowId = (network: Pick<NetworkView, "id" | "scope" | "nodeIds">) =>
    network.scope === "swarm" ? network.id : `${network.nodeIds[0] ?? ""}/${network.id}`;

/** The node a local network lives on; `null` for swarm scope. */
export const networkNode = (network: Pick<NetworkView, "scope" | "nodeIds">) => (network.scope === "local" ? (network.nodeIds[0] ?? null) : null);

/** The 12 characters Docker prints. */
export const shortNetworkId = (id: string) => id.slice(0, 12);

/**
 * A canonical IPv4 CIDR (`10.0.9.0/24`) or an IPv6 one, mirroring the controller: prefix 8 or
 * more, and no host bits set.
 */
export function isCidr(value: string): boolean {
    const [address, prefix, ...rest] = value.split("/");
    if (rest.length > 0 || address === undefined || prefix === undefined || !/^\d{1,3}$/.test(prefix) || String(Number(prefix)) !== prefix) return false;
    const bits = Number(prefix);
    const v4 = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(address);
    if (v4) {
        const octets = v4.slice(1).map(Number);
        if (bits < 8 || bits > 32 || octets.some((octet, index) => octet > 255 || String(octet) !== v4[index + 1])) return false;
        const numeric = octets.reduce((total, octet) => total * 256 + octet, 0);
        const hostSize = 2 ** (32 - bits);
        return numeric % hostSize === 0;
    }
    if (!address.includes(":") || bits < 8 || bits > 128) return false;
    const halves = address.split("::");
    if (halves.length > 2) return false;
    const groups = (part: string | undefined) => (part ? part.split(":") : []);
    const head = groups(halves[0]);
    const tail = groups(halves[1]);
    if (![...head, ...tail].every((group) => /^[0-9a-fA-F]{1,4}$/.test(group))) return false;
    const missing = 8 - head.length - tail.length;
    if (halves.length === 1 ? missing !== 0 : missing < 1) return false;
    const full = [...head, ...Array<string>(missing).fill("0"), ...tail].map((group) => BigInt(`0x${group}`));
    const numeric = full.reduce((total, group) => (total << 16n) + group, 0n);
    return numeric % 2n ** BigInt(128 - bits) === 0n;
}
