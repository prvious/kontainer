import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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

function renderWithProviders(serviceId: string) {
    const queryClient = createTestQueryClient();

    return render(
        <AuthContext value={mockAuth}>
            <QueryClientProvider client={queryClient}>
                <ServiceDetail serviceId={serviceId} />
            </QueryClientProvider>
        </AuthContext>,
    );
}

const replicatedService: ServiceDetailType = {
    id: "svc1",
    name: "web",
    image: "nginx:latest",
    stack: "myapp",
    mode: "replicated",
    replicas: 3,
    createdAt: "2025-01-01T00:00:00Z",
    updatedAt: "2025-01-02T00:00:00Z",
    env: [],
    mounts: [],
    constraints: [],
    ports: [],
    updateStatus: null,
    previousImage: null,
};

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

const zeroReplicaService: ServiceDetailType = {
    ...replicatedService,
    id: "svc-zero",
    replicas: 0,
};

describe("Scale Service", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("shows scale controls for replicated services", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "inspect_service") return replicatedService;
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByText("replicated")).toBeInTheDocument();
        });

        expect(screen.getByLabelText("Decrease replicas")).toBeInTheDocument();
        expect(screen.getByLabelText("Increase replicas")).toBeInTheDocument();
        expect(screen.getByLabelText("Replica count")).toHaveValue(3);
    });

    it("does not show scale controls for global mode services", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "inspect_service") return globalService;
            return undefined;
        });

        renderWithProviders("svc-global");

        await waitFor(() => {
            expect(screen.getByText("global")).toBeInTheDocument();
        });

        expect(screen.queryByLabelText("Decrease replicas")).not.toBeInTheDocument();
        expect(screen.queryByLabelText("Increase replicas")).not.toBeInTheDocument();
    });

    it("clicking + calls scale_service with replicas + 1", async () => {
        const user = userEvent.setup();

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "inspect_service") return replicatedService;
            if (cmd === "scale_service") return { replicas: 4 };
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByLabelText("Increase replicas")).toBeInTheDocument();
        });

        await user.click(screen.getByLabelText("Increase replicas"));

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith("scale_service", {
                url: "http://localhost:3000",
                token: "test-jwt",
                serviceId: "svc1",
                replicas: 4,
            });
        });
    });

    it("clicking - calls scale_service with replicas - 1", async () => {
        const user = userEvent.setup();

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "inspect_service") return replicatedService;
            if (cmd === "scale_service") return { replicas: 2 };
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByLabelText("Decrease replicas")).toBeInTheDocument();
        });

        await user.click(screen.getByLabelText("Decrease replicas"));

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith("scale_service", {
                url: "http://localhost:3000",
                token: "test-jwt",
                serviceId: "svc1",
                replicas: 2,
            });
        });
    });

    it("minus button is disabled when replicas is 0", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "inspect_service") return zeroReplicaService;
            return undefined;
        });

        renderWithProviders("svc-zero");

        await waitFor(() => {
            expect(screen.getByLabelText("Decrease replicas")).toBeInTheDocument();
        });

        expect(screen.getByLabelText("Decrease replicas")).toBeDisabled();
        expect(screen.getByLabelText("Increase replicas")).not.toBeDisabled();
    });
});
