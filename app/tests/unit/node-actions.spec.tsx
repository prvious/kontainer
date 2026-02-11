import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import { NodeActions } from "@/components/node-actions";
import type { NodeItem } from "@/types/nodes";

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

function renderNodeActions(node: NodeItem) {
    const queryClient = createTestQueryClient();
    return render(
        <AuthContext value={mockAuth}>
            <QueryClientProvider client={queryClient}>
                <NodeActions node={node} />
            </QueryClientProvider>
        </AuthContext>,
    );
}

const activeWorker: NodeItem = {
    id: "node1",
    hostname: "worker-1",
    role: "worker",
    status: "ready",
    availability: "active",
    engineVersion: "24.0.7",
    os: "linux",
    architecture: "x86_64",
    ip: "10.0.0.2",
    leader: false,
    updatedAt: new Date().toISOString(),
};

const drainedWorker: NodeItem = {
    ...activeWorker,
    id: "node2",
    hostname: "worker-2",
    availability: "drain",
};

const managerLeader: NodeItem = {
    id: "node3",
    hostname: "manager-1",
    role: "manager",
    status: "ready",
    availability: "active",
    engineVersion: "24.0.7",
    os: "linux",
    architecture: "x86_64",
    ip: "10.0.0.1",
    leader: true,
    updatedAt: new Date().toISOString(),
};

const managerNonLeader: NodeItem = {
    ...managerLeader,
    id: "node4",
    hostname: "manager-2",
    leader: false,
};

describe("NodeActions", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("shows actions menu button", () => {
        renderNodeActions(activeWorker);
        expect(screen.getByLabelText("Actions for worker-1")).toBeInTheDocument();
    });

    it("shows Drain and Pause for active nodes", async () => {
        const user = userEvent.setup();
        renderNodeActions(activeWorker);

        await user.click(screen.getByLabelText("Actions for worker-1"));

        await waitFor(() => {
            expect(screen.getByText("Drain")).toBeInTheDocument();
        });
        expect(screen.getByText("Pause")).toBeInTheDocument();
    });

    it("shows Activate for drained nodes", async () => {
        const user = userEvent.setup();
        renderNodeActions(drainedWorker);

        await user.click(screen.getByLabelText("Actions for worker-2"));

        await waitFor(() => {
            expect(screen.getByText("Activate")).toBeInTheDocument();
        });
    });

    it("shows Promote for workers", async () => {
        const user = userEvent.setup();
        renderNodeActions(activeWorker);

        await user.click(screen.getByLabelText("Actions for worker-1"));

        await waitFor(() => {
            expect(screen.getByText("Promote to Manager")).toBeInTheDocument();
        });
    });

    it("shows Demote for non-leader managers", async () => {
        const user = userEvent.setup();
        renderNodeActions(managerNonLeader);

        await user.click(screen.getByLabelText("Actions for manager-2"));

        await waitFor(() => {
            expect(screen.getByText("Demote to Worker")).toBeInTheDocument();
        });
    });

    it("opens drain dialog when clicking Drain", async () => {
        const user = userEvent.setup();

        renderNodeActions(activeWorker);

        await user.click(screen.getByLabelText("Actions for worker-1"));

        await waitFor(() => {
            expect(screen.getByText("Drain")).toBeInTheDocument();
        });

        await user.click(screen.getByText("Drain"));

        // Should open the drain dialog
        await waitFor(() => {
            expect(screen.getByText("Drain Node: worker-1")).toBeInTheDocument();
        });
    });

    it("calls update_node_availability when clicking Activate", async () => {
        const user = userEvent.setup();
        vi.mocked(invoke).mockResolvedValue({ availability: "active" });

        renderNodeActions(drainedWorker);

        await user.click(screen.getByLabelText("Actions for worker-2"));

        await waitFor(() => {
            expect(screen.getByText("Activate")).toBeInTheDocument();
        });

        await user.click(screen.getByText("Activate"));

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith("update_node_availability", {
                url: "http://localhost:8080",
                token: "test-jwt",
                nodeId: "node2",
                availability: "active",
            });
        });
    });

    it("calls update_node_role when clicking Promote", async () => {
        const user = userEvent.setup();
        vi.mocked(invoke).mockResolvedValue({ role: "manager" });

        renderNodeActions(activeWorker);

        await user.click(screen.getByLabelText("Actions for worker-1"));

        await waitFor(() => {
            expect(screen.getByText("Promote to Manager")).toBeInTheDocument();
        });

        await user.click(screen.getByText("Promote to Manager"));

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith("update_node_role", {
                url: "http://localhost:8080",
                token: "test-jwt",
                nodeId: "node1",
                role: "manager",
            });
        });
    });
});
