import { useState } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { ArrowDownUp, Download, HardDrive, Layers, RefreshCw } from "lucide-react";
import { PageHeader } from "@/components/page-header";
import { ActionButton } from "@/components/kit/action-button";
import { EmptyState, ErrorState } from "@/components/kit/region-states";
import { InlineAlert } from "@/components/kit/inline-alert";
import { notify } from "@/components/kit/notify";
import { TableToolbar } from "@/components/data-table/table-toolbar";
import { PinnedOperations } from "@/components/operations/pinned-operations";
import { bytes } from "@/components/containers/container-format";
import { ImageStorageStrip } from "@/components/images/image-storage-strip";
import { ImagesTable } from "@/components/images/images-table";
import { DeleteImageDialog } from "@/components/images/delete-image-dialog";
import { PullImageDialog } from "@/components/images/pull-image-dialog";
import { IMAGE_SORTS, IMAGE_SORT_LABELS, imageSortFor, imageSortingFor, parseImageSearch, type ImageSearch } from "@/components/images/image-search";
import { useContainers } from "@/hooks/use-containers";
import { useImages, useRemoveImage } from "@/hooks/use-images";
import { useNodes } from "@/hooks/use-nodes";
import { agentNodes } from "@/types/nodes";
import { isSettled, useTrackedOperations } from "@/hooks/use-operation";
import { withIdempotencyKey } from "@/hooks/use-operations";
import { imageLabel, type ImageView } from "@/types/images";

export const Route = createFileRoute("/_authenticated/images/")({
    validateSearch: (search: Record<string, unknown>): ImageSearch => parseImageSearch(search),
    component: ImagesPage,
});

const plural = (count: number, one: string, many = `${one}s`) => `${count} ${count === 1 ? one : many}`;

function ImagesPage() {
    const search = Route.useSearch();
    const navigate = Route.useNavigate();
    const images = useImages();
    const containers = useContainers();
    const nodes = useNodes();
    const remove = useRemoveImage();
    const tracked = useTrackedOperations("images");
    const [deleting, setDeleting] = useState<ImageView | null>(null);
    const [pullOpen, setPullOpen] = useState(false);

    const snapshot = images.data;
    const rows = snapshot?.items ?? [];
    const outcomes = snapshot?.nodes ?? [];
    const silent = outcomes.filter((node) => node.status !== "ok");
    const uniqueBytes = rows.reduce((total, image) => total + image.sizeBytes, 0);

    // Images an in-flight removal acts on fade until the list refreshes.
    const pending = new Set<string>();
    let pulling = false;
    for (const { operation, view } of tracked) {
        const running = !view || !isSettled(view.state);
        if (running) for (const target of operation.targets ?? []) pending.add(target);
        if (running && view?.kind === "image.pull") pulling = true;
    }

    function setSearch(next: ImageSearch) {
        void navigate({ search: next, replace: true });
    }

    function confirmDelete(image: ImageView) {
        const label = imageLabel(image);
        // Docker refuses to delete an image by id while it carries more than one tag, even when unused.
        const force = image.usedBy > 0 || image.repoTags.length > 1;
        remove.mutate(withIdempotencyKey({ imageId: image.imageId, nodeIds: "all" as const, force, label }), {
            onError: (code) =>
                notify.error(`Couldn't delete ${label}`, { code: String(code), message: "The controller did not accept the removal. Nothing was changed." }),
        });
        setDeleting(null);
    }

    const emptyState =
        rows.length === 0 && !search.q ? (
            <EmptyState icon={Layers} title='No images yet' primaryAction={{ label: "PULL IMAGE", icon: Download, onClick: () => setPullOpen(true) }}>
                Images show up here once a node pulls one, whether a service needs it or you pull it yourself.
            </EmptyState>
        ) : (
            <EmptyState title='No images match this search' primaryAction={{ label: "CLEAR SEARCH", onClick: () => setSearch({ sort: search.sort }) }}>
                Nothing matches "{search.q}" by tag, digest, or id.
            </EmptyState>
        );

    const errorState =
        images.error && !snapshot ? (
            <ErrorState
                title="Couldn't load images"
                details={{ code: String(images.error), message: "The controller answered, but the image list request failed. Nothing was changed." }}
                onRetry={() => void images.refetch()}
            />
        ) : undefined;

    const sort = search.sort ?? "created";
    return (
        <div className='flex flex-col gap-5'>
            <PageHeader title='IMAGES' subtitle={`${plural(rows.length, "image")} · ${bytes(uniqueBytes)} total`}>
                <ActionButton variant='outline' icon={RefreshCw} pending={images.isFetching} onClick={() => void images.refetch()}>
                    REFRESH
                </ActionButton>
                <ActionButton variant='primary' icon={Download} pending={pulling} mutates onClick={() => setPullOpen(true)}>
                    {pulling ? "PULLING…" : "PULL IMAGE"}
                </ActionButton>
            </PageHeader>
            {silent.length > 0 && snapshot && (
                <InlineAlert tone='destructive' title={`${silent.map((node) => node.nodeId).join(", ")} didn't answer — their images may be missing`}>
                    Partial results: {outcomes.length - silent.length} of {outcomes.length} nodes responded.
                </InlineAlert>
            )}
            <ImageStorageStrip images={rows} />
            <div className='flex flex-col gap-3'>
                <TableToolbar
                    search={search.q ?? ""}
                    onSearchChange={(q) => setSearch({ ...search, q: q || undefined })}
                    searchPlaceholder='Search images, tags, digest, or id'
                    sortOptions={IMAGE_SORTS.map((value) => ({ value, label: IMAGE_SORT_LABELS[value] }))}
                    sort={sort}
                    onSortChange={(next) => setSearch({ ...search, sort: next === "created" ? undefined : (next as ImageSearch["sort"]) })}
                />
                <PinnedOperations scope='images' />
                <ImagesTable
                    rows={rows}
                    search={search.q ?? ""}
                    sorting={imageSortingFor(search.sort)}
                    onSortingChange={(sorting) => {
                        const next = imageSortFor(sorting);
                        setSearch({ ...search, sort: next === "created" ? undefined : next });
                    }}
                    pending={pending}
                    onDelete={setDeleting}
                    countLabel={String(rows.length)}
                    headerRight={
                        <span className='flex items-center gap-4 font-mono text-3xs font-bold tracking-[1px] text-muted-foreground uppercase'>
                            <span className='flex items-center gap-1.5'>
                                <HardDrive aria-hidden className='size-3' /> {bytes(uniqueBytes)} total
                            </span>
                            <span className='flex items-center gap-1.5'>
                                <ArrowDownUp aria-hidden className='size-3' /> {IMAGE_SORT_LABELS[sort]} {sort === "name" ? "asc" : "desc"}
                            </span>
                        </span>
                    }
                    isFirstLoad={images.isPending}
                    isRefetching={images.isFetching && !images.isPending}
                    emptyState={emptyState}
                    errorState={errorState}
                />
            </div>

            <DeleteImageDialog
                image={deleting}
                containers={containers.data?.items ?? []}
                onOpenChange={(open) => !open && setDeleting(null)}
                onConfirm={confirmDelete}
            />
            <PullImageDialog open={pullOpen} onOpenChange={setPullOpen} nodes={agentNodes(nodes.data?.items ?? [])} />
        </div>
    );
}
