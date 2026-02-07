import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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
    agentUrl: "http://localhost:8080",
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

const stacksResponse = {
    stacks: [
        { name: "myapp", services: 3, runningReplicas: 5, desiredReplicas: 5, updatedAt: "2025-01-02T00:00:00Z" },
        { name: "monitoring", services: 2, runningReplicas: 4, desiredReplicas: 4, updatedAt: "2025-01-01T00:00:00Z" },
    ],
};

describe("Remove Stack", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("shows remove button for each stack on hover", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_stacks") return stacksResponse;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("myapp")).toBeInTheDocument();
        });

        expect(screen.getByLabelText("Remove stack myapp")).toBeInTheDocument();
        expect(screen.getByLabelText("Remove stack monitoring")).toBeInTheDocument();
    });

    it("clicking remove opens confirmation dialog", async () => {
        const user = userEvent.setup();

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_stacks") return stacksResponse;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByLabelText("Remove stack myapp")).toBeInTheDocument();
        });

        await user.click(screen.getByLabelText("Remove stack myapp"));

        await waitFor(() => {
            expect(screen.getByText(/Remove stack "myapp"\?/)).toBeInTheDocument();
        });

        expect(screen.getByText("This will remove all services in the stack. This action cannot be undone.")).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Remove" })).toBeInTheDocument();
    });

    it("confirming remove calls remove_stack invoke", async () => {
        const user = userEvent.setup();

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_stacks") return stacksResponse;
            if (cmd === "remove_stack") return { removed: 3 };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByLabelText("Remove stack myapp")).toBeInTheDocument();
        });

        await user.click(screen.getByLabelText("Remove stack myapp"));

        await waitFor(() => {
            expect(screen.getByRole("button", { name: "Remove" })).toBeInTheDocument();
        });

        await user.click(screen.getByRole("button", { name: "Remove" }));

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith("remove_stack", {
                url: "http://localhost:8080",
                token: "test-jwt",
                stackName: "myapp",
            });
        });
    });

    it("cancelling does not call remove_stack", async () => {
        const user = userEvent.setup();

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_stacks") return stacksResponse;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByLabelText("Remove stack myapp")).toBeInTheDocument();
        });

        await user.click(screen.getByLabelText("Remove stack myapp"));

        await waitFor(() => {
            expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
        });

        await user.click(screen.getByRole("button", { name: "Cancel" }));

        // Wait a bit to make sure no remove_stack call was made
        await waitFor(() => {
            expect(invoke).not.toHaveBeenCalledWith("remove_stack", expect.anything());
        });
    });
});
