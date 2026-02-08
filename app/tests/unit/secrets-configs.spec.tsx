import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import type { SecretItem } from "@/types/secrets";
import type { ConfigItem } from "@/types/configs";
import { useSecrets } from "@/hooks/use-secrets";
import { useConfigs } from "@/hooks/use-configs";
import { useRemoveSecret } from "@/hooks/use-remove-secret";
import { useRemoveConfig } from "@/hooks/use-remove-config";
import { useState } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { AlertCircle, FileText, KeyRound } from "lucide-react";

function SecretsTable({ secrets, onRemove }: { secrets: SecretItem[]; onRemove: (id: string) => void }) {
    return (
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead>Name</TableHead>
                    <TableHead>Labels</TableHead>
                    <TableHead>Created</TableHead>
                    <TableHead className='w-10' />
                </TableRow>
            </TableHeader>
            <TableBody>
                {secrets.map((secret) => (
                    <TableRow key={secret.id}>
                        <TableCell className='font-medium'>{secret.name}</TableCell>
                        <TableCell>
                            {Object.keys(secret.labels).length === 0 ? (
                                <span className='text-xs text-muted-foreground'>-</span>
                            ) : (
                                <div className='flex flex-wrap gap-1'>
                                    {Object.entries(secret.labels).map(([k, v]) => (
                                        <Badge key={k} variant='secondary' className='text-[10px]'>
                                            {k}={v}
                                        </Badge>
                                    ))}
                                </div>
                            )}
                        </TableCell>
                        <TableCell>
                            <span className='text-xs text-muted-foreground'>just now</span>
                        </TableCell>
                        <TableCell>
                            <Button variant='ghost' size='sm' onClick={() => onRemove(secret.id)} data-testid={`remove-secret-${secret.id}`}>
                                Remove
                            </Button>
                        </TableCell>
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}

function ConfigsTable({ configs, onRemove }: { configs: ConfigItem[]; onRemove: (id: string) => void }) {
    return (
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead>Name</TableHead>
                    <TableHead>Labels</TableHead>
                    <TableHead>Size</TableHead>
                    <TableHead>Created</TableHead>
                    <TableHead className='w-10' />
                </TableRow>
            </TableHeader>
            <TableBody>
                {configs.map((config) => (
                    <TableRow key={config.id}>
                        <TableCell className='font-medium'>{config.name}</TableCell>
                        <TableCell>
                            {Object.keys(config.labels).length === 0 ? (
                                <span className='text-xs text-muted-foreground'>-</span>
                            ) : (
                                <div className='flex flex-wrap gap-1'>
                                    {Object.entries(config.labels).map(([k, v]) => (
                                        <Badge key={k} variant='secondary' className='text-[10px]'>
                                            {k}={v}
                                        </Badge>
                                    ))}
                                </div>
                            )}
                        </TableCell>
                        <TableCell>
                            <span className='font-mono text-xs text-muted-foreground'>{config.dataSize} B</span>
                        </TableCell>
                        <TableCell>
                            <span className='text-xs text-muted-foreground'>just now</span>
                        </TableCell>
                        <TableCell>
                            <Button variant='ghost' size='sm' onClick={() => onRemove(config.id)} data-testid={`remove-config-${config.id}`}>
                                Remove
                            </Button>
                        </TableCell>
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}

function LoadingSkeleton() {
    return (
        <Card>
            <CardContent className='p-0'>
                <div className='space-y-0'>
                    {[1, 2, 3, 4, 5].map((i) => (
                        <div key={i} className='flex items-center gap-4 border-b px-4 py-3 last:border-0'>
                            <Skeleton className='h-4 w-32' />
                            <Skeleton className='h-4 w-20' />
                            <Skeleton className='ml-auto h-4 w-16' />
                        </div>
                    ))}
                </div>
            </CardContent>
        </Card>
    );
}

function SecretsConfigsTestComponent() {
    const [activeTab, setActiveTab] = useState<string>("secrets");
    const { data: secrets, isLoading: secretsLoading, error: secretsError } = useSecrets();
    const { data: configs, isLoading: configsLoading, error: configsError } = useConfigs();
    const removeSecret = useRemoveSecret();
    const removeConfig = useRemoveConfig();

    return (
        <Tabs value={activeTab} onValueChange={(v) => setActiveTab(v as string)}>
            <TabsList>
                <TabsTrigger value='secrets'>Secrets</TabsTrigger>
                <TabsTrigger value='configs'>Configs</TabsTrigger>
            </TabsList>

            <TabsContent value='secrets'>
                {secretsLoading ? (
                    <LoadingSkeleton />
                ) : secretsError ? (
                    <div className='text-destructive'>
                        <AlertCircle className='size-5' />
                        <p>Failed to load secrets</p>
                    </div>
                ) : !secrets || secrets.length === 0 ? (
                    <div>
                        <KeyRound className='size-5' />
                        <p>No secrets found</p>
                    </div>
                ) : (
                    <Card>
                        <CardContent className='p-0'>
                            <SecretsTable secrets={secrets} onRemove={(id) => removeSecret.mutate(id)} />
                        </CardContent>
                    </Card>
                )}
            </TabsContent>

            <TabsContent value='configs'>
                {configsLoading ? (
                    <LoadingSkeleton />
                ) : configsError ? (
                    <div className='text-destructive'>
                        <AlertCircle className='size-5' />
                        <p>Failed to load configs</p>
                    </div>
                ) : !configs || configs.length === 0 ? (
                    <div>
                        <FileText className='size-5' />
                        <p>No configs found</p>
                    </div>
                ) : (
                    <Card>
                        <CardContent className='p-0'>
                            <ConfigsTable configs={configs} onRemove={(id) => removeConfig.mutate(id)} />
                        </CardContent>
                    </Card>
                )}
            </TabsContent>
        </Tabs>
    );
}

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

function createTestRouter() {
    const rootRoute = createRootRoute({});
    const indexRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/",
        component: SecretsConfigsTestComponent,
    });
    const routeTree = rootRoute.addChildren([indexRoute]);
    return createRouter({
        routeTree,
        history: createMemoryHistory({ initialEntries: ["/"] }),
    });
}

function renderWithProviders(authOverrides: Partial<AuthContextValue> = {}) {
    const queryClient = createTestQueryClient();
    const auth = { ...mockAuth, ...authOverrides };
    const router = createTestRouter();

    return render(
        <AuthContext value={auth}>
            <QueryClientProvider client={queryClient}>
                <RouterProvider router={router} />
            </QueryClientProvider>
        </AuthContext>,
    );
}

const sampleSecrets: SecretItem[] = [
    {
        id: "sec1",
        name: "db-password",
        labels: { env: "prod" },
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
    },
    {
        id: "sec2",
        name: "api-key",
        labels: {},
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
    },
];

const sampleConfigs: ConfigItem[] = [
    {
        id: "cfg1",
        name: "nginx.conf",
        labels: { app: "web" },
        dataSize: 1024,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
    },
    {
        id: "cfg2",
        name: "app.env",
        labels: {},
        dataSize: 256,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
    },
];

describe("SecretsConfigsPage", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("shows loading skeleton initially", async () => {
        vi.mocked(invoke).mockImplementation(() => new Promise(() => {}));
        renderWithProviders();

        await waitFor(() => {
            const skeletons = document.querySelectorAll('[data-slot="skeleton"]');
            expect(skeletons.length).toBeGreaterThan(0);
        });
    });

    it("renders secrets tab by default with secret names", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_secrets") return { secrets: sampleSecrets };
            if (cmd === "list_configs") return { configs: sampleConfigs };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("db-password")).toBeInTheDocument();
        });

        expect(screen.getByText("api-key")).toBeInTheDocument();
    });

    it("shows label badges on secrets", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_secrets") return { secrets: sampleSecrets };
            if (cmd === "list_configs") return { configs: sampleConfigs };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("env=prod")).toBeInTheDocument();
        });
    });

    it("switches to configs tab and shows config names", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_secrets") return { secrets: sampleSecrets };
            if (cmd === "list_configs") return { configs: sampleConfigs };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("db-password")).toBeInTheDocument();
        });

        fireEvent.click(screen.getByText("Configs"));

        await waitFor(() => {
            expect(screen.getByText("nginx.conf")).toBeInTheDocument();
        });

        expect(screen.getByText("app.env")).toBeInTheDocument();
    });

    it("shows data size for configs", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_secrets") return { secrets: sampleSecrets };
            if (cmd === "list_configs") return { configs: sampleConfigs };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("db-password")).toBeInTheDocument();
        });

        fireEvent.click(screen.getByText("Configs"));

        await waitFor(() => {
            expect(screen.getByText("1024 B")).toBeInTheDocument();
        });
    });

    it("shows empty state for secrets when none exist", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_secrets") return { secrets: [] };
            if (cmd === "list_configs") return { configs: [] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("No secrets found")).toBeInTheDocument();
        });
    });

    it("shows empty state for configs when none exist", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_secrets") return { secrets: [] };
            if (cmd === "list_configs") return { configs: [] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("No secrets found")).toBeInTheDocument();
        });

        fireEvent.click(screen.getByText("Configs"));

        await waitFor(() => {
            expect(screen.getByText("No configs found")).toBeInTheDocument();
        });
    });

    it("shows error state for secrets when fetch fails", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_secrets") throw new Error("connection error");
            if (cmd === "list_configs") return { configs: [] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Failed to load secrets")).toBeInTheDocument();
        });
    });

    it("shows error state for configs when fetch fails", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_secrets") return { secrets: [] };
            if (cmd === "list_configs") throw new Error("connection error");
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("No secrets found")).toBeInTheDocument();
        });

        fireEvent.click(screen.getByText("Configs"));

        await waitFor(() => {
            expect(screen.getByText("Failed to load configs")).toBeInTheDocument();
        });
    });

    it("calls remove_secret invoke when removing a secret", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_secrets") return { secrets: sampleSecrets };
            if (cmd === "list_configs") return { configs: [] };
            if (cmd === "remove_secret") return { removed: true };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("db-password")).toBeInTheDocument();
        });

        fireEvent.click(screen.getByTestId("remove-secret-sec1"));

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith("remove_secret", {
                url: "http://localhost:8080",
                token: "test-jwt",
                secretId: "sec1",
            });
        });
    });

    it("calls remove_config invoke when removing a config", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_secrets") return { secrets: [] };
            if (cmd === "list_configs") return { configs: sampleConfigs };
            if (cmd === "remove_config") return { removed: true };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("No secrets found")).toBeInTheDocument();
        });

        fireEvent.click(screen.getByText("Configs"));

        await waitFor(() => {
            expect(screen.getByText("nginx.conf")).toBeInTheDocument();
        });

        fireEvent.click(screen.getByTestId("remove-config-cfg1"));

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith("remove_config", {
                url: "http://localhost:8080",
                token: "test-jwt",
                configId: "cfg1",
            });
        });
    });

    it("does not fetch when auth is missing", () => {
        vi.mocked(invoke).mockImplementation(async () => ({ secrets: [], configs: [] }));

        renderWithProviders({ token: null, agentUrl: null });

        expect(invoke).not.toHaveBeenCalledWith("list_secrets", expect.anything());
        expect(invoke).not.toHaveBeenCalledWith("list_configs", expect.anything());
    });
});
