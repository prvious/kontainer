import { useMemo, useRef, useState, type ReactNode } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { notify } from "@/components/kit/notify";
import { operationSummary } from "@/components/operations/operation-meta";
import { OperationsContext, type OperationsContextValue, type TrackedOperation } from "@/contexts/operations-context-value";
import { useOperation } from "@/hooks/use-operation";
import type { OperationView } from "@/types/operations";

/** Renders nothing; keeps one tracked operation polled until it settles. */
function OperationWatcher({ id }: { id: string }) {
    useOperation(id);
    return null;
}

/**
 * The operations this window started (mounted in the authenticated layout). When one settles it
 * refreshes the queries it affects and toasts once, keyed by the operation id so the result
 * replaces a loading toast shown for it; succeeded and cancelled ones leave the list, failed and
 * uncertain ones stay pinned until dismissed.
 */
export function OperationsProvider({ children }: { children: ReactNode }) {
    const queryClient = useQueryClient();
    const [tracked, setTracked] = useState<TrackedOperation[]>([]);
    // Written only from callbacks: the operations by id, and the ones already acted on.
    const registry = useRef(new Map<string, TrackedOperation>());
    const handled = useRef(new Set<string>());

    const value = useMemo<OperationsContextValue>(() => {
        function remove(id: string) {
            registry.current.delete(id);
            setTracked((current) => current.filter((operation) => operation.id !== id));
        }
        return {
            tracked,
            track(operation) {
                if (registry.current.has(operation.id)) return;
                registry.current.set(operation.id, operation);
                setTracked((current) => [...current, operation]);
            },
            dismiss: remove,
            settle(view: OperationView) {
                const operation = registry.current.get(view.id);
                if (!operation || handled.current.has(view.id)) return;
                handled.current.add(view.id);
                for (const queryKey of operation.invalidate) void queryClient.invalidateQueries({ queryKey });
                operation.onSettled?.(view);
                switch (view.state) {
                    case "succeeded":
                        notify.success(operation.success?.title ?? operation.title, {
                            description: operation.success?.description ?? operationSummary(view),
                            id: view.id,
                        });
                        remove(view.id);
                        return;
                    case "cancelled":
                        notify.warning(`${operation.title} cancelled`, { description: "It had not started, so nothing changed.", id: view.id });
                        remove(view.id);
                        return;
                    case "failed":
                        notify.error(
                            `${operation.title} failed`,
                            { code: view.errorCode ?? "failed", message: view.errorMessage ?? "The operation failed.", requestId: view.requestId },
                            { id: view.id },
                        );
                        return;
                    default:
                        notify.warning(`${operation.title}: outcome uncertain`, {
                            description: "The controller restarted while this ran. Check the result before retrying.",
                            id: view.id,
                        });
                }
            },
        };
    }, [tracked, queryClient]);

    return (
        <OperationsContext value={value}>
            {children}
            {tracked.map((operation) => (
                <OperationWatcher key={operation.id} id={operation.id} />
            ))}
        </OperationsContext>
    );
}
