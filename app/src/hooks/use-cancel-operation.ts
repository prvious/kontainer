import { useContext } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { CONTROLLER_UNREACHABLE } from "@/contexts/connectivity-context-value";
import { OperationsContext } from "@/contexts/operations-context-value";
import { useAuth } from "@/hooks/use-auth";
import { useConnectivity } from "@/hooks/use-connectivity";
import { authenticatedInvoke } from "@/hooks/authenticated-invoke";
import { operationKey } from "@/hooks/use-operation";
import type { OperationView } from "@/types/operations";

/**
 * Cancel an operation that has not started; the fresh snapshot replaces the polled one. Refused
 * with `controller_unreachable`, unsent, while the controller is unreachable.
 */
export function useCancelOperation() {
    const { session, reportInventoryCustodyError } = useAuth();
    const { reachable } = useConnectivity();
    const queryClient = useQueryClient();
    const operations = useContext(OperationsContext);
    const profileId = session?.profileId ?? "";
    return useMutation({
        mutationFn: async (operationId: string) => {
            if (!reachable) throw CONTROLLER_UNREACHABLE;
            return authenticatedInvoke<OperationView>("cancel_operation", profileId, { operationId }, reportInventoryCustodyError);
        },
        onSuccess: (view) => {
            queryClient.setQueryData(operationKey(profileId, view.id), view);
            operations?.settle(view);
        },
    });
}
