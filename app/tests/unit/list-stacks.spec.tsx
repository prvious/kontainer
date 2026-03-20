import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import { StacksList } from "@/components/stacks-list";

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
    agentUrl: "http://localhost:3000",
    isLoading: false,
    login: vi.fn(),
    logout: vi.fn(),
};

function renderWithProviders() {
    const queryClient = createTestQueryClient();

    return render(
        <AuthContext value={mockAuth}>
            <QueryClientProvider client={queryClient}>
                <StacksList />
            </QueryClientProvider>
        </AuthContext>,
    );
}

describe("List Stacks", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("renders stacks with names and service counts", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_stacks")
                return {
                    stacks: [
                        { name: "myapp", services: 3, runningReplicas: 5, desiredReplicas: 5, updatedAt: "2025-01-02T00:00:00Z" },
                        { name: "monitoring", services: 2, runningReplicas: 4, desiredReplicas: 4, updatedAt: "2025-01-01T00:00:00Z" },
                    ],
                };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("myapp")).toBeInTheDocument();
        });

        expect(screen.getByText("monitoring")).toBeInTheDocument();
        expect(screen.getByText("3")).toBeInTheDocument();
        expect(screen.getByText("2")).toBeInTheDocument();
    });

    it("shows replica badges with correct counts", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_stacks")
                return {
                    stacks: [{ name: "myapp", services: 2, runningReplicas: 3, desiredReplicas: 5, updatedAt: "2025-01-01T00:00:00Z" }],
                };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("3/5")).toBeInTheDocument();
        });
    });

    it("shows empty state when no stacks exist", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_stacks") return { stacks: [] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("No stacks found")).toBeInTheDocument();
        });
    });

    it("shows error state on failure", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_stacks") throw new Error("connection refused");
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Failed to load stacks")).toBeInTheDocument();
        });
    });

    it("renders table headers", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_stacks")
                return {
                    stacks: [{ name: "myapp", services: 1, runningReplicas: 1, desiredReplicas: 1, updatedAt: "2025-01-01T00:00:00Z" }],
                };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Stack")).toBeInTheDocument();
        });

        expect(screen.getByText("Services")).toBeInTheDocument();
        expect(screen.getByText("Replicas")).toBeInTheDocument();
        expect(screen.getByText("Updated")).toBeInTheDocument();
    });
});
