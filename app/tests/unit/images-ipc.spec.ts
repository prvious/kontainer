import { describe, expect, it } from "vitest";
import fixture from "../fixtures/images-ipc.json";
import { ipcFixture } from "../ipc-fixture";
import { imageLabel, imageName, nodePullsFromProgress, referenceProblem, shortImageId, type ImageListView } from "@/types/images";
import { imageSortFor, imageSortingFor, parseImageSearch } from "@/components/images/image-search";
import { containerUsesImage, diskUsageBytes } from "@/components/images/image-format";
import { nodeProgress } from "@/components/images/pull-progress";
import type { OperationView } from "@/types/operations";

// The shared fixture must fit the view Rust's own tests serialize to it.
const list = () => ipcFixture<ImageListView>(fixture.imageList);

describe("images IPC contract", () => {
    it("reads per-node pulls from the operation progress and drops malformed entries", () => {
        const pulls = nodePullsFromProgress(fixture.pullProgress);
        expect(pulls.map((pull) => [pull.nodeId, pull.status])).toEqual([
            ["manager", "done"],
            ["worker-1", "pulling"],
            ["worker-2", "failed"],
        ]);
        expect(pulls[2]?.error).toBe("unauthorized: authentication required");
        expect(nodePullsFromProgress({ nodes: [{ nodeId: "x", status: "exploded" }, "junk"] })).toEqual([]);
        expect(nodePullsFromProgress(null)).toEqual([]);
    });
});

describe("image names", () => {
    it("use the first tag, <none> for dangling images, and the 12-character id", () => {
        const [nginx, dangling] = list().items;
        expect(imageName(nginx!)).toEqual({ repository: "nginx", tag: "1.27" });
        expect(imageName(dangling!)).toEqual({ repository: "<none>", tag: "<none>" });
        expect(imageName({ repoTags: ["localhost:5000/team/app:v1"] })).toEqual({ repository: "localhost:5000/team/app", tag: "v1" });
        // Docker's containerd image store appends the digest to the tag.
        const pinned = { repoTags: [`nginx:alpine@sha256:${"d".repeat(64)}`], imageId: nginx!.imageId };
        expect(imageName(pinned)).toEqual({ repository: "nginx", tag: "alpine" });
        expect(imageLabel(pinned)).toBe("nginx:alpine");
        expect(containerUsesImage({ image: "nginx:alpine", nodeId: "manager" }, { ...nginx!, repoTags: pinned.repoTags })).toBe(true);
        expect(imageLabel(nginx!)).toBe("nginx:1.27");
        expect(imageLabel(dangling!)).toBe("cccccccccccc");
        expect(shortImageId(nginx!.imageId)).toBe("aaaaaaaaaaaa");
    });
});

describe("reference validation mirrors the controller", () => {
    it.each([
        "alpine",
        "alpine:3.20",
        "library/alpine:3.20",
        "ghcr.io/acme/billing:2.4.0",
        "localhost:5000/team/app",
        "registry.example.com:443/a/b/c:v1_2.3-rc",
        "my-org/my__app.name:tag",
        `alpine@sha256:${"a".repeat(64)}`,
    ])("accepts %s", (reference) => expect(referenceProblem(reference)).toBeNull());

    it.each([
        ["", "empty_or_too_long"],
        ["a".repeat(513), "empty_or_too_long"],
        ["Alpine", "invalid_name"],
        ["alpine:", "invalid_tag"],
        ["alpine:-bad", "invalid_tag"],
        ["alpine:has space", "invalid_tag"],
        ["alpine@sha256:abc", "invalid_digest"],
        ["-alpine", "invalid_name"],
        ["alpine-", "invalid_name"],
        ["a..b/c", "invalid_registry"],
        ["bad_host.io:99999999/x", "invalid_registry"],
        ["alp!ne", "invalid_name"],
        ["ghcr.io/", "invalid_name"],
    ])("rejects %j as %s", (reference, problem) => expect(referenceProblem(reference)).toBe(problem));
});

describe("image helpers", () => {
    it("count disk usage once per node that holds an image", () => {
        expect(diskUsageBytes(list().items)).toBe(192_000_000 * 2 + 7_800_000);
    });

    it("match containers by tag, pinned digest, or id on nodes that hold the image", () => {
        const nginx = { ...list().items[0]!, repoDigests: [`nginx@sha256:${"d".repeat(64)}`] };
        expect(containerUsesImage({ image: "nginx:1.27", nodeId: "manager" }, nginx)).toBe(true);
        expect(containerUsesImage({ image: `nginx:1.27@sha256:${"d".repeat(64)}`, nodeId: "worker-1" }, nginx)).toBe(true);
        expect(containerUsesImage({ image: `other@sha256:${"d".repeat(64)}`, nodeId: "worker-1" }, nginx)).toBe(true);
        expect(containerUsesImage({ image: nginx.imageId, nodeId: "manager" }, nginx)).toBe(true);
        expect(containerUsesImage({ image: "nginx:1.27", nodeId: "worker-2" }, nginx)).toBe(false);
        expect(containerUsesImage({ image: "nginx", nodeId: "manager" }, nginx)).toBe(false);
        expect(containerUsesImage({ image: "nginx", nodeId: "manager" }, { ...nginx, repoTags: ["nginx:latest"] })).toBe(true);
    });

    it("validate search params with defaults and map sorts both ways", () => {
        expect(parseImageSearch({})).toEqual({});
        expect(parseImageSearch({ q: "nginx", sort: "size" })).toEqual({ q: "nginx", sort: "size" });
        expect(parseImageSearch({ q: " ", sort: "created" })).toEqual({});
        expect(parseImageSearch({ sort: "bogus" })).toEqual({});
        expect(imageSortingFor(undefined)).toEqual([{ id: "created", desc: true }]);
        expect(imageSortFor(imageSortingFor("size"))).toBe("size");
        expect(imageSortFor([{ id: "usedBy", desc: true }])).toBeUndefined();
    });
});

describe("pull progress rows", () => {
    const view = (state: OperationView["state"], progress: Record<string, unknown> | null, errorMessage: string | null = null) =>
        ({ state, progress, errorMessage }) as OperationView;

    it("let a retry replace the failure it retries", () => {
        const attempts = [
            { operationId: "first", nodeIds: ["manager", "worker-1", "worker-2"] },
            { operationId: "retry", nodeIds: ["worker-2"] },
        ];
        const retried = { nodes: [{ nodeId: "worker-2", status: "done", layersDone: 3, layersTotal: 3, bytes: 3_600_000, totalBytes: 3_600_000 }] };
        const rows = nodeProgress(attempts, [view("failed", fixture.pullProgress, "The pull failed on 1 of 3 nodes."), view("succeeded", retried)]);
        expect(rows.map((row) => [row.nodeId, row.state])).toEqual([
            ["manager", "done"],
            ["worker-1", "failed"],
            ["worker-2", "done"],
        ]);
        expect(rows[1]?.error).toBe("The pull failed on 1 of 3 nodes.");
    });

    it("wait while queued and show the verbatim registry error", () => {
        expect(nodeProgress([{ operationId: "a", nodeIds: ["manager"] }], [undefined])[0]?.state).toBe("queued");
        const rows = nodeProgress([{ operationId: "a", nodeIds: ["worker-2"] }], [view("running", fixture.pullProgress)]);
        expect(rows[0]).toMatchObject({ state: "failed", error: "unauthorized: authentication required" });
    });
});
