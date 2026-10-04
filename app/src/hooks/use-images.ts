import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useAuth } from "@/hooks/use-auth";
import { useAppSettings } from "@/hooks/use-app-settings";
import { useQueriesPaused } from "@/hooks/use-queries-paused";
import { authenticatedInvoke } from "@/hooks/authenticated-invoke";
import { useStartOperation, type WithIdempotencyKey } from "@/hooks/use-operations";
import type { ImageListView } from "@/types/images";
import { DEFAULT_APP_SETTINGS } from "@/types/settings";
import { containersKey } from "@/hooks/use-containers";

export const imagesKey = (profileId: string) => ["images", profileId] as const;

/** Every image on the cluster, merged by id, polled at the refresh interval. */
export function useImages() {
    const { session, reportInventoryCustodyError } = useAuth();
    const paused = useQueriesPaused();
    const settings = useAppSettings();
    const profileId = session?.profileId ?? "";
    return useQuery({
        queryKey: imagesKey(profileId),
        queryFn: () => authenticatedInvoke<ImageListView>("list_images", profileId, {}, reportInventoryCustodyError),
        enabled: Boolean(session) && !paused,
        refetchInterval: settings.data?.refreshInterval ?? DEFAULT_APP_SETTINGS.refreshInterval ?? 5000,
        retry: false,
        placeholderData: keepPreviousData,
    });
}

export interface RemoveImageVariables extends WithIdempotencyKey {
    imageId: string;
    /** Node ids, or every node that has it. */
    nodeIds: string[] | "all";
    force: boolean;
    /** For the operation title only; not sent. */
    label: string;
}

/** One image removal, tracked under the images scope; the row fades while it runs. */
export function useRemoveImage() {
    const { session } = useAuth();
    const profileId = session?.profileId ?? "";
    return useStartOperation<RemoveImageVariables>({
        command: "remove_image",
        scope: "images",
        title: ({ label }) => `Deleting ${label}`,
        success: ({ label }) => ({ title: `Deleted ${label}`, description: "Deletes are final; pull the image again to use it." }),
        targets: ({ imageId }) => [imageId],
        invalidate: [imagesKey(profileId), containersKey(profileId)],
        args: ({ imageId, nodeIds, force, idempotencyKey }) => ({ imageId, nodeIds, force, idempotencyKey }),
    });
}

export interface PullImageVariables extends WithIdempotencyKey {
    reference: string;
    nodeIds: string[];
}

const nodesText = (count: number) => `${count} ${count === 1 ? "node" : "nodes"}`;

/** One pull across nodes, tracked under the images scope; its progress carries each node. */
export function usePullImage() {
    const { session } = useAuth();
    const profileId = session?.profileId ?? "";
    return useStartOperation<PullImageVariables>({
        command: "pull_image",
        scope: "images",
        title: ({ reference }) => `Pulling ${reference}`,
        success: ({ reference, nodeIds }) => ({ title: `Pulled ${reference}`, description: `Ready on ${nodesText(nodeIds.length)}.` }),
        invalidate: [imagesKey(profileId)],
        args: ({ reference, nodeIds, idempotencyKey }) => ({ reference, nodeIds, idempotencyKey }),
    });
}
