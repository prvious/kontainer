import { useContext } from "react";
import { ConnectivityContext } from "@/contexts/connectivity-context-value";

/** Controller reachability from the authenticated layout's health probe. */
export function useConnectivity() {
    return useContext(ConnectivityContext);
}
