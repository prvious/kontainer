import { useContext } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { OperationsContext } from "@/contexts/operations-context-value";
import { useAuth } from "@/hooks/use-auth";
import { authenticatedInvoke } from "@/hooks/authenticated-invoke";
import { operationKey } from "@/hooks/use-operation";
import { operationFromIpc } from "@/types/operations";

/** Cancel an operation that has not started; the fresh snapshot replaces the polled one. */
export function useCancelOperation() {
    const { session, reportInventoryCustodyError } = useAuth();
    const queryClient = useQueryClient();
    const operations = useContext(OperationsContext);
    const profileId = session?.profileId ?? "";
    return useMutation({
        mutationFn: (operationId: string) => authenticatedInvoke("cancel_operation", profileId, { operationId }, operationFromIpc, reportInventoryCustodyError),
        onSuccess: (view) => {
            queryClient.setQueryData(operationKey(profileId, view.id), view);
            operations?.settle(view);
        },
    });
}
