import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import {
    controllerSettingsKey,
    nodesQueries,
    useClusterSummary,
    useControllerSettings,
    useNodes,
    useSamples,
    useUpdateControllerSettings,
} from "@/hooks/use-nodes";
import fixture from "../fixtures/nodes-ipc.json";

const profileId = "018f2e34-0000-7000-8000-0000000000bb";
const reportInventoryCustodyError = vi.fn();
const auth = { reportInventoryCustodyError, session: { profileId }, expired: false } as unknown as AuthContextValue;

function createWrapper(value: AuthContextValue = auth) {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const wrapper = ({ children }: { children: React.ReactNode }) => (
        <AuthContext value={value}>
            <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
        </AuthContext>
    );
    return { queryClient, wrapper };
}

function mockCommands(handlers: Record<string, (args: unknown) => unknown>) {
    vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
        if (command === "load_app_settings") return { refreshInterval: 5000 };
        if (command in handlers) return handlers[command](args);
        throw new Error(`unexpected ${command}`);
    });
}

describe("node membership queries", () => {
    beforeEach(() => vi.clearAllMocks());

    it("keys the list by profile and reads it with only the profile id", async () => {
        mockCommands({ list_nodes: () => fixture.nodeList });
        const { queryClient, wrapper } = createWrapper();
        const { result } = renderHook(() => useNodes(), { wrapper });
        await waitFor(() => expect(result.current.data?.items).toHaveLength(3));
        expect(invoke).toHaveBeenCalledWith("list_nodes", { profileId });
        expect(queryClient.getQueryData(nodesQueries.all(profileId))).toEqual(fixture.nodeList);
    });

    it("reads the cluster summary under the nodes key so node refetches refresh it too", async () => {
        mockCommands({ get_cluster_summary: () => fixture.clusterSummary });
        const { queryClient, wrapper } = createWrapper();
        const { result } = renderHook(() => useClusterSummary(), { wrapper });
        await waitFor(() => expect(result.current.data?.nodesTotal).toBe(3));
        expect(invoke).toHaveBeenCalledWith("get_cluster_summary", { profileId });
        expect(queryClient.getQueryData([...nodesQueries.all(profileId), "summary"])).toEqual(fixture.clusterSummary);
    });

    it("reads cluster and node series under the samples keys", async () => {
        mockCommands({ get_cluster_samples: () => fixture.clusterSamples, get_node_samples: () => fixture.clusterSamples });
        const { queryClient, wrapper } = createWrapper();
        const cluster = renderHook(() => useSamples("cluster", "5m", "60s"), { wrapper });
        const node = renderHook(() => useSamples("manager", "5m", "60s"), { wrapper });
        await waitFor(() => expect(cluster.result.current.data).toBeDefined());
        await waitFor(() => expect(node.result.current.data).toBeDefined());
        expect(invoke).toHaveBeenCalledWith("get_cluster_samples", { profileId, window: "5m", bucket: "60s" });
        expect(invoke).toHaveBeenCalledWith("get_node_samples", { profileId, nodeId: "manager", window: "5m", bucket: "60s" });
        expect(queryClient.getQueryData(["nodes", profileId, "samples", "cluster", "5m", "60s"])).toEqual(fixture.clusterSamples);
    });

    it("pauses while the session-expired dialog is up", async () => {
        mockCommands({ list_nodes: () => fixture.nodeList });
        const { wrapper } = createWrapper({ ...auth, expired: true } as AuthContextValue);
        const { result } = renderHook(() => useNodes(), { wrapper });
        await new Promise((resolve) => setTimeout(resolve, 20));
        expect(result.current.fetchStatus).toBe("idle");
        expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "list_nodes")).toHaveLength(0);
    });

    it("reports custody failures once and surfaces closed codes", async () => {
        mockCommands({ list_nodes: () => Promise.reject("relogin_required") });
        const { wrapper } = createWrapper();
        const { result } = renderHook(() => useNodes(), { wrapper });
        await waitFor(() => expect(result.current.isError).toBe(true));
        expect(result.current.error).toBe("relogin_required");
        expect(reportInventoryCustodyError).toHaveBeenCalledExactlyOnceWith(profileId, "relogin_required");
    });

    it("reads and updates the controller settings", async () => {
        mockCommands({
            get_settings: () => fixture.settings,
            update_settings: (args) => ({ metricsRetentionDays: (args as { metricsRetentionDays: number }).metricsRetentionDays }),
        });
        const { queryClient, wrapper } = createWrapper();
        const settings = renderHook(() => useControllerSettings(), { wrapper });
        await waitFor(() => expect(settings.result.current.data).toEqual({ metricsRetentionDays: 7 }));
        const update = renderHook(() => useUpdateControllerSettings(), { wrapper });
        await act(() => update.result.current.mutateAsync(30));
        expect(invoke).toHaveBeenCalledWith("update_settings", { profileId, metricsRetentionDays: 30 });
        expect(queryClient.getQueryData(controllerSettingsKey(profileId))).toEqual({ metricsRetentionDays: 30 });
    });
});
