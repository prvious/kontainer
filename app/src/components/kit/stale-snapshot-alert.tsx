import { RefreshCw } from "lucide-react";
import { InlineAlert } from "@/components/kit/inline-alert";
import { clockTime } from "@/lib/time";

/** The parts of a TanStack query result the alert reads. */
interface RefreshedQuery {
    error: unknown;
    dataUpdatedAt: number;
    isFetching: boolean;
    refetch: () => unknown;
}

/**
 * A refetch failed after a good load, so the page still shows the last good snapshot: say when it
 * was taken, give the error code, and offer a retry. Pages render it while `query.error &&
 * query.data`, as `DataTable`'s `stale` banner (which fades the rows) or above a card grid.
 */
export function StaleSnapshotAlert({ query, className }: { query: RefreshedQuery; className?: string }) {
    return (
        <InlineAlert
            tone='warning'
            className={className}
            title={`Couldn't refresh — showing the snapshot from ${clockTime(query.dataUpdatedAt, true)}`}
            action={{ label: "RETRY NOW", icon: RefreshCw, pending: query.isFetching, onClick: () => void query.refetch() }}
        >
            {String(query.error)} · Nothing below is live until a refresh succeeds.
        </InlineAlert>
    );
}
