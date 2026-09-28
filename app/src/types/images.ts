/**
 * Images across the cluster, crossing the Tauri boundary.
 *
 * Mirrors `app/src-tauri/src/session/manager_images.rs`; the shared fixture at
 * `app/tests/fixtures/images-ipc.json` keeps both sides in sync.
 */
import { exactRecord } from "@/types/inventory";
import { nodeOutcomeFromIpc, type NodeOutcomeView } from "@/types/containers";

export interface ImageView {
    /** `sha256:<64 hex>`. */
    imageId: string;
    repoTags: string[];
    repoDigests: string[];
    /** Counted once, however many nodes hold it. */
    sizeBytes: number;
    createdAt: string;
    /** Containers using it, summed across nodes. */
    usedBy: number;
    dangling: boolean;
    nodeIds: string[];
}

export interface ImageListView {
    items: ImageView[];
    nodes: NodeOutcomeView[];
    snapshotAt: string;
}

export const PULL_STATUSES = ["pulling", "done", "failed"] as const;
export type PullStatus = (typeof PULL_STATUSES)[number];

/** One node's pull, from the `image.pull` operation's progress document. */
export interface NodePull {
    nodeId: string;
    status: PullStatus;
    layersDone: number;
    layersTotal: number;
    bytes: number;
    totalBytes: number;
    /** The registry's or daemon's message, verbatim. */
    error?: string;
}

const TIMESTAMP = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;
const IMAGE_ID = /^sha256:[0-9a-f]{64}$/;
const isTextList = (value: unknown): value is string[] => Array.isArray(value) && value.every((entry) => typeof entry === "string");
const isCount = (value: unknown): value is number => typeof value === "number" && Number.isInteger(value) && value >= 0;

function imageFromIpc(value: unknown): ImageView {
    const data = exactRecord(value, ["imageId", "repoTags", "repoDigests", "sizeBytes", "createdAt", "usedBy", "dangling", "nodeIds"]);
    if (
        typeof data.imageId !== "string" ||
        !IMAGE_ID.test(data.imageId) ||
        !isTextList(data.repoTags) ||
        !isTextList(data.repoDigests) ||
        !isCount(data.sizeBytes) ||
        typeof data.createdAt !== "string" ||
        !TIMESTAMP.test(data.createdAt) ||
        !isCount(data.usedBy) ||
        typeof data.dangling !== "boolean" ||
        !isTextList(data.nodeIds) ||
        data.nodeIds.length === 0
    )
        throw "invalid_response";
    return data as unknown as ImageView;
}

export function imageListFromIpc(value: unknown): ImageListView {
    const data = exactRecord(value, ["items", "nodes", "snapshotAt"]);
    if (
        !Array.isArray(data.items) ||
        data.items.length > 4000 ||
        !Array.isArray(data.nodes) ||
        typeof data.snapshotAt !== "string" ||
        !TIMESTAMP.test(data.snapshotAt)
    )
        throw "invalid_response";
    return { items: data.items.map(imageFromIpc), nodes: data.nodes.map(nodeOutcomeFromIpc), snapshotAt: data.snapshotAt };
}

/**
 * The per-node pulls in an `image.pull` progress document. The document is opaque at the IPC
 * boundary, so a malformed entry is dropped rather than trusted.
 */
export function nodePullsFromProgress(progress: Record<string, unknown> | null | undefined): NodePull[] {
    const nodes = progress?.nodes;
    if (!Array.isArray(nodes)) return [];
    return nodes.filter((entry): entry is NodePull => {
        if (typeof entry !== "object" || entry === null) return false;
        const pull = entry as Record<string, unknown>;
        return (
            typeof pull.nodeId === "string" &&
            PULL_STATUSES.some((status) => status === pull.status) &&
            isCount(pull.layersDone) &&
            isCount(pull.layersTotal) &&
            isCount(pull.bytes) &&
            isCount(pull.totalBytes) &&
            (pull.error === undefined || typeof pull.error === "string")
        );
    });
}

/**
 * A tag without the digest Docker's containerd image store appends (`nginx:alpine@sha256:…`
 * reads as `nginx:alpine`).
 */
export const plainTag = (tag: string) => tag.split("@")[0] ?? tag;

/** The first tag's repository and tag, `<none>` for a dangling image. */
export function imageName(image: Pick<ImageView, "repoTags">): { repository: string; tag: string } {
    const first = image.repoTags[0];
    if (!first) return { repository: "<none>", tag: "<none>" };
    const plain = plainTag(first);
    const colon = plain.lastIndexOf(":");
    const slash = plain.lastIndexOf("/");
    return colon > slash ? { repository: plain.slice(0, colon), tag: plain.slice(colon + 1) } : { repository: plain, tag: "latest" };
}

/** `name:tag` as typed to confirm a delete; the short id when the image has no tag. */
export function imageLabel(image: Pick<ImageView, "repoTags" | "imageId">): string {
    const { repository, tag } = imageName(image);
    return image.repoTags.length > 0 ? `${repository}:${tag}` : shortImageId(image.imageId);
}

/** The 12-character id Docker prints. */
export const shortImageId = (imageId: string) => imageId.replace(/^sha256:/, "").slice(0, 12);

export type ReferenceProblem = "empty_or_too_long" | "invalid_registry" | "invalid_name" | "invalid_tag" | "invalid_digest";

const PATH_COMPONENT = /^[a-z0-9]+(?:(?:\.|_|__|-+)[a-z0-9]+)*$/;
const DOMAIN_LABEL = /^[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?$/;
const TAG = /^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$/;
const DIGEST = /^sha256:[0-9a-f]{64}$/;

function validDomain(value: string): boolean {
    const colon = value.lastIndexOf(":");
    const host = colon >= 0 ? value.slice(0, colon) : value;
    const port = colon >= 0 ? value.slice(colon + 1) : null;
    return (port === null || /^\d{1,5}$/.test(port)) && host.length > 0 && host.split(".").every((label) => DOMAIN_LABEL.test(label));
}

/**
 * `[host[:port]/]name[:tag|@digest]` with the Docker reference grammar, mirroring the
 * controller's check (it answers 400 for the same inputs). `null` means valid.
 */
export function referenceProblem(input: string): ReferenceProblem | null {
    const value = input.trim();
    if (value.length === 0 || value.length > 512) return "empty_or_too_long";
    const at = value.indexOf("@");
    const rest = at >= 0 ? value.slice(0, at) : value;
    if (at >= 0 && !DIGEST.test(value.slice(at + 1))) return "invalid_digest";
    const colon = rest.lastIndexOf(":");
    const hasTag = colon > rest.lastIndexOf("/");
    const name = hasTag ? rest.slice(0, colon) : rest;
    if (hasTag && !TAG.test(rest.slice(colon + 1))) return "invalid_tag";
    if (name.length === 0 || name.length > 255) return "invalid_name";
    const components = name.split("/");
    const first = components[0] ?? "";
    const hasDomain = components.length > 1 && (first.includes(".") || first.includes(":") || first === "localhost" || /[A-Z]/.test(first));
    if (hasDomain) {
        if (!validDomain(first)) return "invalid_registry";
        components.shift();
    }
    if (components.length === 0 || !components.every((component) => PATH_COMPONENT.test(component))) return "invalid_name";
    return null;
}

export const REFERENCE_PROBLEMS: Record<ReferenceProblem, string> = {
    empty_or_too_long: "Enter an image reference, like alpine:3.20.",
    invalid_registry: "The registry host isn't valid.",
    invalid_name: "Repository names are lowercase letters, digits, and . _ - separators.",
    invalid_tag: "Tags are up to 128 letters, digits, and . _ -, not starting with . or -.",
    invalid_digest: "Digests look like sha256: followed by 64 lowercase hex characters.",
};
