/**
 * Node-scoped volumes crossing the Tauri boundary.
 *
 * Mirrors `app/src-tauri/src/session/manager_volumes.rs`; the shared fixture at
 * `app/tests/fixtures/volumes-ipc.json` keeps both sides in sync.
 */
import type { NodeOutcomeView } from "@/types/containers";

export interface VolumeView {
    nodeId: string;
    name: string;
    driver: string;
    /** Host path of a `local` volume. */
    mountPoint: string;
    createdAt: string | null;
    labels: Record<string, string>;
    /** Containers on the node (any state) that mount it. */
    usedBy: number;
    /** Up to 16 of their names. */
    users: string[];
    /** `null` until the node's first disk sample, or when the driver can't say. */
    sizeBytes: number | null;
}

export interface VolumeListView {
    items: VolumeView[];
    nodes: NodeOutcomeView[];
    snapshotAt: string;
}

/** Docker's volume name rule, capped at 255: `[a-zA-Z0-9][a-zA-Z0-9_.-]+`. */
export const VOLUME_NAME = /^[a-zA-Z0-9][a-zA-Z0-9_.-]{1,254}$/;

/** Stable row id: a name is unique per node, not across the cluster. */
export const volumeRowId = (volume: Pick<VolumeView, "nodeId" | "name">) => `${volume.nodeId}/${volume.name}`;

export type LabelProblem = "format" | "duplicate" | "too_long" | "too_many";

/** Parse one `key=value` label as typed; `null` when it can't be added. */
export function parseLabel(input: string, existing: Record<string, string>): { key: string; value: string } | LabelProblem {
    const separator = input.indexOf("=");
    const key = (separator >= 0 ? input.slice(0, separator) : input).trim();
    const value = separator >= 0 ? input.slice(separator + 1).trim() : "";
    if (key === "" || /[\p{Cc}]/u.test(input)) return "format";
    if (key.length > 256 || value.length > 256) return "too_long";
    if (key in existing) return "duplicate";
    if (Object.keys(existing).length >= 64) return "too_many";
    return { key, value };
}

export const LABEL_PROBLEMS: Record<LabelProblem, string> = {
    format: "Labels are key=value, with a key.",
    duplicate: "That key is already set.",
    too_long: "Keys and values are at most 256 characters.",
    too_many: "A volume carries at most 64 labels.",
};
