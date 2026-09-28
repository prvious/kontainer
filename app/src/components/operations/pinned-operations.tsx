import { OperationRow } from "@/components/operations/operation-row";
import { useOperations } from "@/hooks/use-operations";

/**
 * Operations this window started for `scope`, pinned under the page header (04.S3). Failed ones
 * stay until dismissed; succeeded ones leave after their toast.
 */
export function PinnedOperations({ scope }: { scope: string }) {
    const { tracked } = useOperations();
    const pinned = tracked.filter((operation) => operation.scope === scope);
    if (pinned.length === 0) return null;
    return (
        <div aria-label='Operations' className='grid grid-cols-1 gap-3 lg:grid-cols-2'>
            {pinned.map((operation) => (
                <OperationRow key={operation.id} operation={operation} />
            ))}
        </div>
    );
}
