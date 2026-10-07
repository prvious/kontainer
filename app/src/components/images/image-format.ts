import type { ContainerRow } from "@/hooks/use-containers";
import { plainTag, shortImageId, type ImageView } from "@/types/images";

/** On-disk bytes across the cluster: an image held by three nodes takes its size three times. */
export function diskUsageBytes(images: ImageView[]): number {
    return images.reduce((total, image) => total + image.sizeBytes * image.nodeIds.length, 0);
}

/**
 * Whether a container runs this image. Containers report the reference they were created from
 * (Swarm tasks pin `name:tag@sha256:…`), so it matches by id, tag, or repository digest.
 */
export function containerUsesImage(container: Pick<ContainerRow, "image" | "nodeId">, image: ImageView): boolean {
    if (!image.nodeIds.includes(container.nodeId)) return false;
    const reference = container.image;
    if (reference === image.imageId || reference === shortImageId(image.imageId)) return true;
    const [named, digest] = reference.split("@");
    const tagged = named && named.lastIndexOf(":") > named.lastIndexOf("/") ? named : `${named}:latest`;
    if (image.repoTags.some((repoTag) => plainTag(repoTag) === tagged)) return true;
    return digest !== undefined && image.repoDigests.some((repoDigest) => repoDigest.endsWith(`@${digest}`));
}
