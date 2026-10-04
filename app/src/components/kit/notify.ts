import { toast } from "sonner";
import { errorDetailsText, type ErrorDetails } from "@/components/kit/error-details";
import { CONTROLLER_UNREACHABLE, PAUSED_OFFLINE } from "@/contexts/connectivity-context-value";

interface ToastAction {
    label: string;
    onClick: () => void;
}

/**
 * Toast `y1xaD` and its error / warning / loading variants: bottom-right, 5 s, optional action.
 * Success and failure toasts stack; a loading toast is replaced in place by passing its id.
 */
export const notify = {
    success(title: string, options: { description?: string; action?: ToastAction; id?: string | number } = {}) {
        return toast.success(title, { description: options.description, action: options.action, id: options.id, duration: 5000 });
    },
    warning(title: string, options: { description?: string; action?: ToastAction; id?: string | number } = {}) {
        return toast.warning(title, { description: options.description, action: options.action, id: options.id, duration: 5000 });
    },
    /**
     * Failure toast: reason code, message, and request id, with COPY DETAILS unless another action is given.
     * A change refused unsent because the controller is unreachable says so, whatever the caller's message.
     */
    error(title: string, reported: ErrorDetails, options: { action?: ToastAction; id?: string | number } = {}) {
        const details = reported.code === CONTROLLER_UNREACHABLE ? { ...reported, message: PAUSED_OFFLINE.reason } : reported;
        const description = [details.message, [details.code, details.requestId ? `req ${details.requestId}` : null].filter(Boolean).join(" · ")].join("\n");
        return toast.error(title, {
            description,
            id: options.id,
            duration: 5000,
            action: options.action ?? {
                label: "COPY DETAILS",
                onClick: () => void navigator.clipboard.writeText(errorDetailsText(details)).catch(() => {}),
            },
        });
    },
    loading(title: string, options: { description?: string; action?: ToastAction; id?: string | number } = {}) {
        return toast.loading(title, { description: options.description, action: options.action, id: options.id, duration: Infinity });
    },
};
