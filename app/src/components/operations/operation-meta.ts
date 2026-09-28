import type { OperationPhase } from "@/components/kit/operation-status";
import { shortDuration } from "@/lib/time";
import { progressCounts, type OperationView } from "@/types/operations";

const RECOVERED_AFTER_RESTART = "recovered_after_restart";

export function operationPhase(view: OperationView | undefined): OperationPhase {
    if (!view || view.state === "recoverable") return "queued";
    return view.state;
}

function elapsedSeconds(view: OperationView, now: number): number | null {
    if (!view.startedAt) return null;
    const end = view.finishedAt ? Date.parse(view.finishedAt) : now;
    return (end - Date.parse(view.startedAt)) / 1000;
}

function countsText(view: OperationView): string | null {
    const counts = progressCounts(view.progress);
    if (!counts) return null;
    return counts.failed > 0 ? `${counts.done} of ${counts.total} · ${counts.failed} failed` : `${counts.done} of ${counts.total}`;
}

/** The design's status line: `RUNNING · 4 of 5 · 12s`, `FAILED · code — message · req id`. */
export function operationMeta(view: OperationView | undefined, now: number): string {
    if (!view || view.state === "recoverable") return "QUEUED · waiting for a worker";
    const elapsed = elapsedSeconds(view, now);
    const duration = elapsed === null ? null : shortDuration(elapsed);
    const request = view.requestId ? `req ${view.requestId}` : null;
    switch (view.state) {
        case "running":
            return ["RUNNING", countsText(view) ?? "in progress", duration].filter(Boolean).join(" · ");
        case "succeeded":
            return ["SUCCEEDED", countsText(view), duration ? `finished in ${duration}` : null].filter(Boolean).join(" · ");
        case "failed":
            return ["FAILED", [view.errorCode ?? "failed", view.errorMessage].filter(Boolean).join(" — "), request].filter(Boolean).join(" · ");
        case "cancelled":
            return "CANCELLED · before it started";
        case "uncertain":
            return [
                "UNCERTAIN",
                view.errorCode === RECOVERED_AFTER_RESTART
                    ? "the controller restarted while this ran — check the result"
                    : (view.errorCode ?? "check the result"),
                request,
            ]
                .filter(Boolean)
                .join(" · ");
    }
}

/** 0–1 for the track; succeeded fills it, queued leaves it empty. */
export function operationTrack(view: OperationView | undefined): number | null {
    if (!view) return null;
    if (view.state === "succeeded") return 1;
    const counts = progressCounts(view.progress);
    return counts ? counts.done / counts.total : null;
}

/** Success toast description: counts and duration when known. */
export function operationSummary(view: OperationView): string | undefined {
    const elapsed = view.startedAt && view.finishedAt ? shortDuration((Date.parse(view.finishedAt) - Date.parse(view.startedAt)) / 1000) : null;
    const parts = [countsText(view), elapsed ? `finished in ${elapsed}` : null].filter(Boolean);
    return parts.length > 0 ? parts.join(" · ") : undefined;
}
