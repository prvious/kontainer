import { createContext } from "react";
import type { ProbeView } from "@/types/session";

export interface ConnectivityContextValue {
    /** The latest probe: the failed attempt while one is failing, else the last answer; `undefined` before the first. */
    probe: ProbeView | null | undefined;
    /** False after three unreachable attempts in a row, until a probe answers. */
    reachable: boolean;
    /** When reachability last came back, epoch ms; null until it has. */
    reconnectedAt: number | null;
    /** Probe again now. */
    retry: () => void;
    retrying: boolean;
}

/** Why a mutation is disabled while the controller is unreachable (03.S2). */
export const PAUSED_OFFLINE = {
    title: "Changes are paused while offline",
    reason: "Kontainer can't reach the controller, so it won't send a change it can't confirm. Retry the connection first.",
};

/** Outside the authenticated layout nothing probes, so everything counts as reachable. */
export const ConnectivityContext = createContext<ConnectivityContextValue>({
    probe: undefined,
    reachable: true,
    reconnectedAt: null,
    retry: () => {},
    retrying: false,
});
