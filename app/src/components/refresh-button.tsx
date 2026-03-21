import { useCallback, useMemo } from "react";
import { useQueryClient, useIsFetching } from "@tanstack/react-query";
import { RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";

interface RefreshButtonProps {
    queryKeys: string[][];
}

export function RefreshButton({ queryKeys }: RefreshButtonProps) {
    const queryClient = useQueryClient();

    const matchesKeys = useMemo(
        () => (query: { queryKey: readonly unknown[] }) => queryKeys.some((key) => key.every((k, i) => query.queryKey[i] === k)),
        [queryKeys],
    );

    const isFetching = useIsFetching({ predicate: matchesKeys });

    const handleRefresh = useCallback(async () => {
        try {
            await queryClient.invalidateQueries({ predicate: matchesKeys });
        } catch {
            // Individual queries handle their own errors
        }
    }, [queryClient, matchesKeys]);

    return (
        <Button variant='outline' size='sm' disabled={isFetching > 0} onClick={handleRefresh} className='h-7 font-mono text-[11px]'>
            <RefreshCw className={`size-3.5 ${isFetching > 0 ? "animate-spin" : ""}`} />
            REFRESH
        </Button>
    );
}
