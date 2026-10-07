export interface ErrorDetails {
    /** Stable reason code, e.g. `engine_unavailable`; an HTTP status may prefix it. */
    code: string;
    message: string;
    requestId?: string | null;
    status?: number | null;
}

export function errorDetailsText({ code, message, requestId, status }: ErrorDetails): string {
    return [`code: ${status ? `${status} ` : ""}${code}`, requestId ? `request id: ${requestId}` : null, `message: ${message}`].filter(Boolean).join("\n");
}
