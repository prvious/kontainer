/**
 * Node-scoped volumes crossing the Tauri boundary.
 *
 * Mirrors `app/src-tauri/src/session/manager_volumes.rs`; the shared fixture at
 * `app/tests/fixtures/volumes-ipc.json` keeps both sides in sync.
 */
import { exactRecord } from "@/types/inventory";
import { nodeOutcomeFromIpc, type NodeOutcomeView } from "@/types/containers";

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

const TIMESTAMP = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;
const isCount = (value: unknown): value is number => typeof value === "number" && Number.isInteger(value) && value >= 0;
const isTextList = (value: unknown): value is string[] => Array.isArray(value) && value.every((entry) => typeof entry === "string");
const isLabels = (value: unknown): value is Record<string, string> =>
    typeof value === "object" && value !== null && !Array.isArray(value) && Object.values(value).every((entry) => typeof entry === "string");

/** Docker's volume name rule, capped at 255: `[a-zA-Z0-9][a-zA-Z0-9_.-]+`. */
export const VOLUME_NAME = /^[a-zA-Z0-9][a-zA-Z0-9_.-]{1,254}$/;

function volumeFromIpc(value: unknown): VolumeView {
    const data = exactRecord(value, ["nodeId", "name", "driver", "mountPoint", "createdAt", "labels", "usedBy", "users", "sizeBytes"]);
    if (
        typeof data.nodeId !== "string" ||
        typeof data.name !== "string" ||
        !VOLUME_NAME.test(data.name) ||
        typeof data.driver !== "string" ||
        typeof data.mountPoint !== "string" ||
        !(data.createdAt === null || (typeof data.createdAt === "string" && TIMESTAMP.test(data.createdAt))) ||
        !isLabels(data.labels) ||
        !isCount(data.usedBy) ||
        !isTextList(data.users) ||
        !(data.sizeBytes === null || isCount(data.sizeBytes))
    )
        throw "invalid_response";
    return data as unknown as VolumeView;
}

export function volumeListFromIpc(value: unknown): VolumeListView {
    const data = exactRecord(value, ["items", "nodes", "snapshotAt"]);
    if (
        !Array.isArray(data.items) ||
        data.items.length > 4000 ||
        !Array.isArray(data.nodes) ||
        typeof data.snapshotAt !== "string" ||
        !TIMESTAMP.test(data.snapshotAt)
    )
        throw "invalid_response";
    return { items: data.items.map(volumeFromIpc), nodes: data.nodes.map(nodeOutcomeFromIpc), snapshotAt: data.snapshotAt };
}

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
