import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import { ServiceDetail } from "@/components/service-detail";
import type { ServiceDetail as ServiceDetailType } from "@/types/service-detail";

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

function renderWithProviders(serviceId: string, authOverrides: Partial<AuthContextValue> = {}) {
    const queryClient = createTestQueryClient();
    const auth = { ...mockAuth, ...authOverrides };

    return render(
        <AuthContext value={auth}>
            <QueryClientProvider client={queryClient}>
                <ServiceDetail serviceId={serviceId} />
            </QueryClientProvider>
        </AuthContext>,
    );
}

const fullService: ServiceDetailType = {
    id: "svc1",
    name: "web",
    image: "nginx:latest",
    stack: "myapp",
    mode: "replicated",
    replicas: 3,
    createdAt: "2025-01-01T00:00:00Z",
    updatedAt: "2025-01-02T00:00:00Z",
    env: [
        { key: "NODE_ENV", value: "production" },
        { key: "DEBUG", value: "true" },
    ],
    mounts: [{ type: "bind", source: "/host/data", target: "/app/data", readOnly: true }],
    constraints: ["node.role==manager"],
    ports: [{ publishedPort: 8080, targetPort: 80, protocol: "tcp", publishMode: "ingress" }],
    updateStatus: {
        state: "completed",
        startedAt: "2025-01-02T00:00:00Z",
        completedAt: "2025-01-02T00:01:00Z",
        message: "update completed",
    },
    previousImage: null,
};

describe("ServiceDetail", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("shows loading skeleton initially", () => {
        vi.mocked(invoke).mockImplementation(() => new Promise(() => {}));
        renderWithProviders("svc1");

        const skeletons = document.querySelectorAll('[data-slot="skeleton"]');
        expect(skeletons.length).toBeGreaterThan(0);
    });

    it("renders all sections for a full service", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "inspect_service") return fullService;
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByText("Overview")).toBeInTheDocument();
        });

        // Overview
        expect(screen.getByText("web")).toBeInTheDocument();
        expect(screen.getByText("nginx:latest")).toBeInTheDocument();
        expect(screen.getByText("replicated")).toBeInTheDocument();
        expect(screen.getByLabelText("Replica count")).toHaveValue(3);
        expect(screen.getByText("myapp")).toBeInTheDocument();

        // Ports
        expect(screen.getByText("Ports")).toBeInTheDocument();
        expect(screen.getByText("8080")).toBeInTheDocument();
        expect(screen.getByText("80")).toBeInTheDocument();
        expect(screen.getByText("tcp")).toBeInTheDocument();
        expect(screen.getByText("ingress")).toBeInTheDocument();

        // Environment
        expect(screen.getByText("Environment")).toBeInTheDocument();
        expect(screen.getByText("NODE_ENV")).toBeInTheDocument();
        expect(screen.getByText("production")).toBeInTheDocument();

        // Mounts
        expect(screen.getByText("Mounts")).toBeInTheDocument();
        expect(screen.getByText("/host/data")).toBeInTheDocument();
        expect(screen.getByText("/app/data")).toBeInTheDocument();
        expect(screen.getByText("read-only")).toBeInTheDocument();

        // Constraints
        expect(screen.getByText("Constraints")).toBeInTheDocument();
        expect(screen.getByText("node.role==manager")).toBeInTheDocument();

        // Update status
        expect(screen.getByText("Update Status")).toBeInTheDocument();
        expect(screen.getByText("completed")).toBeInTheDocument();
        expect(screen.getByText("update completed")).toBeInTheDocument();
    });

    it("shows error state when fetch fails", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "inspect_service") throw new Error("network error");
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByText("Failed to load service details")).toBeInTheDocument();
        });
    });

    it("renders empty sections with None text", async () => {
        const minimalService: ServiceDetailType = {
            id: "svc-min",
            name: "simple",
            image: "alpine:3",
            stack: "",
            mode: "replicated",
            replicas: 1,
            createdAt: "2025-01-01T00:00:00Z",
            updatedAt: "2025-01-01T00:00:00Z",
            env: [],
            mounts: [],
            constraints: [],
            ports: [],
            updateStatus: null,
            previousImage: null,
        };

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "inspect_service") return minimalService;
            return undefined;
        });

        renderWithProviders("svc-min");

        await waitFor(() => {
            expect(screen.getByText("Overview")).toBeInTheDocument();
        });

        const noneTexts = screen.getAllByText("None");
        expect(noneTexts.length).toBe(4); // ports, env, mounts, constraints
    });

    it("renders global mode without replica count", async () => {
        const globalService: ServiceDetailType = {
            id: "svc-global",
            name: "monitoring",
            image: "prometheus:v2",
            stack: "",
            mode: "global",
            replicas: null,
            createdAt: "2025-01-01T00:00:00Z",
            updatedAt: "2025-01-01T00:00:00Z",
            env: [],
            mounts: [],
            constraints: [],
            ports: [],
            updateStatus: null,
            previousImage: null,
        };

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "inspect_service") return globalService;
            return undefined;
        });

        renderWithProviders("svc-global");

        await waitFor(() => {
            expect(screen.getByText("global")).toBeInTheDocument();
        });

        expect(screen.queryByLabelText("Replica count")).not.toBeInTheDocument();
    });

    it("does not fetch when auth is missing", () => {
        vi.mocked(invoke).mockImplementation(async () => fullService);

        renderWithProviders("svc1", { token: null, agentUrl: null });

        expect(invoke).not.toHaveBeenCalledWith("inspect_service", expect.anything());
    });
});
