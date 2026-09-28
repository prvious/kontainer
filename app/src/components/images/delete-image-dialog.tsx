import { Trash2 } from "lucide-react";
import { ConfirmDialog } from "@/components/kit/confirm-dialog";
import { containerUsesImage } from "@/components/images/image-format";
import type { ContainerRow } from "@/hooks/use-containers";
import { imageLabel, type ImageView } from "@/types/images";

const MAX_NAMES = 6;

/**
 * Delete confirmation (03.S7): the impact first ("In use by N containers: …", gathered from the
 * containers list), then typed confirmation of the full `name:tag` when containers use it; unused
 * images get a plain confirm. It targets every node that has the image.
 */
export function DeleteImageDialog({
    image,
    containers,
    onOpenChange,
    onConfirm,
}: {
    image: ImageView | null;
    containers: ContainerRow[];
    onOpenChange: (open: boolean) => void;
    onConfirm: (image: ImageView) => void;
}) {
    const label = image ? imageLabel(image) : "";
    const inUse = image !== null && image.usedBy > 0;
    const users = image ? containers.filter((container) => !container.stale && containerUsesImage(container, image)).map((container) => container.name) : [];
    const count = image?.usedBy ?? 0;
    const names = users.slice(0, MAX_NAMES).join(", ") + (users.length > MAX_NAMES ? ` and ${users.length - MAX_NAMES} more` : "");
    const nodes = image?.nodeIds.length ?? 0;

    return (
        <ConfirmDialog
            open={image !== null}
            onOpenChange={onOpenChange}
            title={`Delete image ${label}?`}
            description={
                inUse
                    ? `The image is removed from every node that has it (${nodes}). Running containers keep working, but can't be rescheduled until the image is pulled again.`
                    : `The image is removed from every node that has it (${nodes}). Nothing uses it.`
            }
            impact={inUse ? `In use by ${count} ${count === 1 ? "container" : "containers"}${names ? `: ${names}` : ""}` : undefined}
            confirmationText={inUse ? label : undefined}
            confirmLabel={inUse ? "DELETE IMAGE" : "DELETE"}
            confirmIcon={Trash2}
            onConfirm={() => {
                if (image) onConfirm(image);
            }}
        />
    );
}
