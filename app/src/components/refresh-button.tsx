import { useCallback } from "react";
import { useQueryClient, useIsFetching } from "@tanstack/react-query";
import { RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";

interface RefreshButtonProps {
    queryKeys: string[][];
}

export function RefreshButton({ queryKeys }: RefreshButtonProps) {
    const queryClient = useQueryClient();
    const isFetching = useIsFetching({
        predicate: (query) => queryKeys.some((key) => key.every((k, i) => query.queryKey[i] === k)),
    });

    const handleRefresh = useCallback(() => {
        for (const key of queryKeys) {
            queryClient.invalidateQueries({ queryKey: key });
        }
    }, [queryClient, queryKeys]);

    return (
        <Button variant='outline' size='sm' disabled={isFetching > 0} onClick={handleRefresh} className='h-7 font-mono text-[11px]'>
            <RefreshCw className={`size-3.5 ${isFetching > 0 ? "animate-spin" : ""}`} />
            REFRESH
        </Button>
    );
}
