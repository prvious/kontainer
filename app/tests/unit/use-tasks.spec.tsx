import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useTasks } from "@/hooks/use-tasks";
import { AuthContext } from "@/contexts/auth-context-value";
import { SettingsContext } from "@/contexts/settings-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import type { SettingsContextValue } from "@/contexts/settings-context-value";
import type { TaskItem } from "@/types/tasks";

function createTestQueryClient() {
    return new QueryClient({
        defaultOptions: {
            queries: { retry: false, gcTime: 0 },
        },
    });
}

const mockAuth: AuthContextValue = {
    user: { id: "1", name: "Test", email: "test@test.com", avatarUrl: "", provider: "github" },
    token: "test-jwt",
    expiresAt: Math.floor(Date.now() / 1000) + 9999,
    agentUrl: "http://localhost:8080",
    isLoading: false,
    login: vi.fn(),
    logout: vi.fn(),
};

const mockSettings: SettingsContextValue = {
    refreshInterval: 5000,
    setRefreshInterval: vi.fn(),
};

function renderHookWithProviders(authOverrides: Partial<AuthContextValue> = {}, settingsOverrides: Partial<SettingsContextValue> = {}) {
    const queryClient = createTestQueryClient();
    const auth = { ...mockAuth, ...authOverrides };
    const settings = { ...mockSettings, ...settingsOverrides };

    const wrapper = ({ children }: { children: React.ReactNode }) => (
        <AuthContext value={auth}>
            <SettingsContext value={settings}>
                <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
            </SettingsContext>
        </AuthContext>
    );

    return renderHook(() => useTasks(), { wrapper });
}

const sampleTasks: TaskItem[] = [
    {
        id: "task1",
        serviceId: "svc1",
        serviceName: "web",
        slot: 1,
        nodeId: "node1",
        nodeName: "worker-1",
        state: "running",
        desiredState: "running",
        exitCode: null,
        error: "",
        timestamp: new Date().toISOString(),
        containerId: "abc123",
        image: "nginx:latest",
    },
    {
        id: "task2",
        serviceId: "svc2",
        serviceName: "api",
        slot: 1,
        nodeId: "node2",
        nodeName: "worker-2",
        state: "failed",
        desiredState: "running",
        exitCode: 1,
        error: "container exited",
        timestamp: new Date().toISOString(),
        containerId: "def456",
        image: "node:20",
    },
];

describe("useTasks", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("fetches tasks successfully", async () => {
        vi.mocked(invoke).mockResolvedValue({ tasks: sampleTasks });

        const { result } = renderHookWithProviders();

        await waitFor(() => {
            expect(result.current.isSuccess).toBe(true);
        });

        expect(result.current.data).toEqual(sampleTasks);
        expect(invoke).toHaveBeenCalledWith("list_tasks", {
            url: "http://localhost:8080",
            token: "test-jwt",
        });
    });

    it("returns error when fetch fails", async () => {
        vi.mocked(invoke).mockRejectedValue(new Error("network error"));

        const { result } = renderHookWithProviders();

        await waitFor(() => {
            expect(result.current.isError).toBe(true);
        });

        expect(result.current.error).toBeTruthy();
    });

    it("does not fetch when token is missing", () => {
        vi.mocked(invoke).mockResolvedValue({ tasks: [] });

        const { result } = renderHookWithProviders({ token: null });

        expect(result.current.isLoading).toBe(false);
        expect(result.current.fetchStatus).toBe("idle");
        expect(invoke).not.toHaveBeenCalled();
    });

    it("does not fetch when agentUrl is missing", () => {
        vi.mocked(invoke).mockResolvedValue({ tasks: [] });

        const { result } = renderHookWithProviders({ agentUrl: null });

        expect(result.current.isLoading).toBe(false);
        expect(result.current.fetchStatus).toBe("idle");
        expect(invoke).not.toHaveBeenCalled();
    });

    it("respects refresh interval from settings", async () => {
        vi.mocked(invoke).mockResolvedValue({ tasks: sampleTasks });

        const { result } = renderHookWithProviders({}, { refreshInterval: 10000 });

        await waitFor(() => {
            expect(result.current.isSuccess).toBe(true);
        });

        expect(result.current.data).toEqual(sampleTasks);
    });
});
