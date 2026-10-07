import { createContext } from "react";
import type { QueryKey } from "@tanstack/react-query";
import type { OperationView } from "@/types/operations";

/** An operation this window started and is watching until it settles. */
export interface TrackedOperation {
    id: string;
    /** Resource scope a page pins, e.g. `services` or `containers`. */
    scope: string;
    /** What the row and the toast call it, e.g. `Restarting web`. */
    title: string;
    /** Queries to refresh once it settles. */
    invalidate: QueryKey[];
    /** Success toast text when it differs from the title. */
    success?: { title: string; description?: string };
    /** Row ids the operation acts on, so tables can mark them while it runs. */
    targets?: string[];
    /** Called once with the settled snapshot, after the toast (e.g. to close a dialog). */
    onSettled?: (view: OperationView) => void;
}

export interface OperationsContextValue {
    tracked: TrackedOperation[];
    track(operation: TrackedOperation): void;
    dismiss(id: string): void;
    /** Called with every settled snapshot; acts once per operation. */
    settle(operation: OperationView): void;
}

export const OperationsContext = createContext<OperationsContextValue | null>(null);
