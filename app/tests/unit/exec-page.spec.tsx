import { useState } from "react";
import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRouter, RouterProvider } from "@tanstack/react-router";
import { routeTree } from "@/routeTree.gen";
import { AuthContext, type AuthContextValue } from "@/contexts/auth-context-value";
import containers from "../fixtures/containers-ipc.json";
import operations from "../fixtures/operations-ipc.json";

vi.mock("sonner", () => ({
    toast: Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn(), warning: vi.fn(), loading: vi.fn(), dismiss: vi.fn() }),
    Toaster: () => null,
}));

// ghostty-web draws on a canvas from WebAssembly; jsdom has neither. The fake records what the
// terminal was given and lets a test type into it.
const terminals = vi.hoisted(() => [] as FakeTerminalShape[]);
interface FakeTerminalShape {
    options: { disableStdin?: boolean; fontSize?: number };
    cols: number;
    rows: number;
    written: string[];
    type(data: string): void;
}
vi.mock("ghostty-web", () => {
    class Terminal {
        options: Record<string, unknown>;
        cols = 80;
        rows = 24;
        written: string[] = [];
        #onData: ((data: string) => void) | null = null;
        constructor(options: Record<string, unknown>) {
            this.options = { ...options };
            terminals.push(this as unknown as FakeTerminalShape);
        }
        loadAddon(addon: { activate(terminal: Terminal): void }) {
            addon.activate(this);
        }
        open() {}
        write(data: string | Uint8Array) {
            this.written.push(typeof data === "string" ? data : new TextDecoder().decode(data));
        }
        onData(listener: (data: string) => void) {
            this.#onData = listener;
        }
        type(data: string) {
            this.#onData?.(data);
        }
        dispose() {}
    }
    class FitAddon {
        #terminal: Terminal | null = null;
        activate(terminal: Terminal) {
            this.#terminal = terminal;
        }
        fit() {
            if (!this.#terminal) return;
            this.#terminal.cols = 132;
            this.#terminal.rows = 40;
        }
    }
    return { Terminal, FitAddon, init: async () => {} };
});

const session = {
    profileId: "018f2e34-0000-7000-8000-0000000000bb",
    origin: "https://controller.example/",
    clusterId: "cluster",
    principalId: "principal",
    email: "owner@example.com",
    displayName: "Owner",
    label: "Production",
    accessExpiresAtUnixMs: 2_000_000_000_000,
};
const auth = {
    session,
    isLoading: false,
    expired: false,
    issue: null,
    reportInventoryCustodyError: vi.fn(),
    logout: vi.fn(),
    retryCommit: vi.fn(),
    retryRestore: vi.fn(),
} as unknown as AuthContextValue;

const [web] = containers.containerList.items;
const sessionId = (index: number) => `018f2e34-0000-7000-8000-00000000e00${index}`;

type Channel = { onmessage: (message: unknown) => void };
let channels: Channel[] = [];
let openAnswers: (() => unknown)[] = [];

function mockCommands(overrides: Record<string, (args: unknown) => unknown> = {}) {
    channels = [];
    vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
        if (command in overrides) return overrides[command](args);
        switch (command) {
            case "exec_open": {
                channels.push((args as { channel: Channel }).channel);
                const answer = openAnswers.shift();
                return answer ? answer() : sessionId(channels.length);
            }
            case "exec_write":
            case "exec_resize":
            case "exec_close":
                return null;
            case "list_containers":
                return containers.containerList;
            case "load_app_settings":
                return { refreshInterval: 60_000 };
            case "controller_health":
                return { status: "ready" };
            case "session_connections":
                return [];
            default:
                return null;
        }
    });
}

interface RouterLocation {
    state: { location: { pathname: string; search: unknown } };
}

function Harness({ path, onRouter }: { path: string; onRouter: (router: RouterLocation) => void }) {
    const [router] = useState(() => {
        const created = createRouter({ routeTree, context: { auth }, history: createMemoryHistory({ initialEntries: [path] }) });
        onRouter(created);
        return created;
    });
    return <RouterProvider router={router} context={{ auth }} />;
}

function renderPage(path = `/exec?node=${web.nodeId}&container=${web.containerId}`) {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    let router: RouterLocation | null = null;
    render(
        <AuthContext value={auth}>
            <QueryClientProvider client={queryClient}>
                <Harness path={path} onRouter={(created) => (router = created)} />
            </QueryClientProvider>
        </AuthContext>,
    );
    return { location: () => (router as RouterLocation | null)?.state.location };
}

async function push(event: unknown, channel = 0) {
    await act(async () => channels[channel].onmessage(event));
}

const calls = (command: string) => vi.mocked(invoke).mock.calls.filter(([name]) => name === command);

describe("Exec page", () => {
    beforeAll(() => {
        // The picker's list scrolls its highlighted item into view; jsdom lays nothing out.
        Element.prototype.scrollIntoView = vi.fn();
    });
    afterAll(() => {
        delete (Element.prototype as { scrollIntoView?: unknown }).scrollIntoView;
    });
    beforeEach(() => {
        vi.clearAllMocks();
        terminals.length = 0;
        openAnswers = [];
        mockCommands();
    });

    it("opens a shell from a container row's link, streams output, sends keys and the fitted size", async () => {
        const { location } = renderPage();
        await waitFor(() =>
            expect(invoke).toHaveBeenCalledWith(
                "exec_open",
                expect.objectContaining({ profileId: session.profileId, nodeId: web.nodeId, containerId: web.containerId, shell: null, cols: 120, rows: 32 }),
            ),
        );
        await waitFor(() => expect(location()?.search).toEqual({}));
        expect(await screen.findByRole("tab", { name: new RegExp(web.name) })).toHaveAttribute("aria-selected", "true");
        expect(screen.getByTestId(`exec-tab-dot-${web.name}`)).toHaveAttribute("data-tone", "warning");

        await push({ type: "started", shell: "/bin/sh" });
        await push({ type: "output", data: btoa("/ # ") });
        expect(screen.getByRole("status", { name: "Session status" })).toHaveTextContent(`Connected to ${web.name}`);
        expect(screen.getByRole("status", { name: "Session status" })).toHaveTextContent("1 active session");
        expect(screen.getByTestId(`exec-tab-dot-${web.name}`)).toHaveAttribute("data-tone", "success");
        expect(await screen.findByText(/1 TERMINALS/)).toBeInTheDocument();

        await waitFor(() => expect(terminals).toHaveLength(1));
        const [terminal] = terminals;
        expect(terminal.written.join("")).toContain("/ # ");
        await act(async () => terminal.type("ls\r"));
        expect(invoke).toHaveBeenCalledWith("exec_write", { sessionId: sessionId(1), data: "ls\r" });
        await waitFor(() => expect(invoke).toHaveBeenCalledWith("exec_resize", { sessionId: sessionId(1), cols: 132, rows: 40 }));
    });

    it("opens a shell from a running container's row; a stopped one's Exec is off", async () => {
        const { location } = renderPage("/containers");
        const row = (name: string) => screen.getByText(name).closest("tr") as HTMLElement;
        await screen.findByText("scratch");
        expect(within(row("scratch")).getByRole("button", { name: "Exec" })).toHaveAttribute("aria-disabled", "true");
        await userEvent.setup().click(within(row(web.name)).getByRole("button", { name: "Exec" }));
        await waitFor(() => expect(invoke).toHaveBeenCalledWith("exec_open", expect.objectContaining({ nodeId: web.nodeId, containerId: web.containerId })));
        expect(location()?.pathname).toBe("/exec");
        expect(await screen.findByRole("tab", { name: new RegExp(web.name) })).toBeInTheDocument();
    });

    it("freezes an ended shell, keeps its output, and reconnects or closes the tab", async () => {
        renderPage();
        await waitFor(() => expect(channels).toHaveLength(1));
        await push({ type: "started", shell: "/bin/bash" });
        await push({ type: "output", data: btoa("bye\r\n") });
        await waitFor(() => expect(terminals).toHaveLength(1));
        await push({ type: "exit", code: 0, reason: "exited" });

        expect(await screen.findByText("Session ended — exit 0")).toBeInTheDocument();
        expect(screen.getByRole("separator")).toHaveTextContent("exit 0");
        expect(screen.getByRole("separator")).not.toHaveTextContent("exit code unknown");
        expect(screen.getByRole("status", { name: "Session status" })).toHaveTextContent(`Disconnected from ${web.name} · exit 0`);
        expect(screen.getByTestId("terminal")).toHaveAttribute("data-ended", "true");
        const [terminal] = terminals;
        expect(terminal.options.disableStdin).toBe(true);
        expect(terminal.written[terminal.written.length - 1]).toBe("\u001b[?25l");
        expect(terminal.written.join("")).toContain("bye");
        await act(async () => terminal.type("nope"));
        expect(calls("exec_write")).toHaveLength(0);

        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: /RECONNECT/ }));
        await waitFor(() => expect(channels).toHaveLength(2));
        await push({ type: "started", shell: "/bin/bash" }, 1);
        expect(screen.queryByText(/Session ended/)).not.toBeInTheDocument();
        expect(screen.getAllByRole("tab")).toHaveLength(1);

        await user.click(screen.getByRole("button", { name: `Close ${web.name}` }));
        expect(await screen.findByText("No shells open")).toBeInTheDocument();
        expect(invoke).toHaveBeenCalledWith("exec_close", { sessionId: sessionId(2) });
    });

    it("says the exit code is unknown when a shell was cut off", async () => {
        renderPage();
        await waitFor(() => expect(channels).toHaveLength(1));
        await push({ type: "started", shell: "/bin/sh" });
        await push({ type: "exit", code: null, reason: "idle_timeout" });
        expect(await screen.findByText("Session ended — idle timeout")).toBeInTheDocument();
        expect(screen.getByRole("separator")).toHaveTextContent("exit code unknown");
    });

    it("says the shell may still be running when the session gave up on it", async () => {
        renderPage();
        await waitFor(() => expect(channels).toHaveLength(1));
        await push({ type: "started", shell: "/bin/sh" });
        await push({ type: "exit", code: null, reason: "abandoned" });
        expect(await screen.findByText("Session ended — the shell may still be running in the container")).toBeInTheDocument();
    });

    it("ends a shell whose tab closes after the open answered but before it started", async () => {
        renderPage();
        const tab = await screen.findByRole("tab", { name: new RegExp(web.name) });
        await waitFor(() => expect(channels).toHaveLength(1));
        // Let the open's answer land; no `started` event comes.
        await act(() => new Promise((resolve) => setTimeout(resolve, 0)));
        expect(tab).toHaveAttribute("aria-keyshortcuts", "Delete");
        expect(screen.getByRole("button", { name: `Close ${web.name}` })).toHaveAttribute("tabindex", "-1");
        act(() => tab.focus());
        await userEvent.setup().keyboard("{Delete}");
        expect(await screen.findByText("No shells open")).toBeInTheDocument();
        expect(invoke).toHaveBeenCalledWith("exec_close", { sessionId: sessionId(1) });
    });

    it("sends keystrokes one at a time, in the order they were typed", async () => {
        const answers: (() => void)[] = [];
        mockCommands({ exec_write: () => new Promise((resolve) => answers.push(() => resolve(null))) });
        renderPage();
        await waitFor(() => expect(channels).toHaveLength(1));
        await push({ type: "started", shell: "/bin/sh" });
        await waitFor(() => expect(terminals).toHaveLength(1));
        const [terminal] = terminals;
        await act(async () => {
            terminal.type("l");
            terminal.type("s");
        });
        await waitFor(() => expect(calls("exec_write")).toHaveLength(1));
        await act(() => new Promise((resolve) => setTimeout(resolve, 0)));
        expect(calls("exec_write")).toHaveLength(1);
        await act(async () => answers[0]());
        await waitFor(() => expect(calls("exec_write")).toHaveLength(2));
        expect(calls("exec_write").map(([, args]) => (args as { data: string }).data)).toEqual(["l", "s"]);
    });

    it("starts a stopped container and then opens the shell, or goes to its logs", async () => {
        const stopped = { ...web, state: "exited" };
        mockCommands({
            list_containers: () => ({ ...containers.containerList, items: [stopped, containers.containerList.items[1]] }),
            container_action: () => operations.admitted,
            get_operation: () => ({ ...operations.operation, kind: "container.start", state: "succeeded", finishedAt: operations.operation.updatedAt }),
        });
        const { location } = renderPage();
        await waitFor(() => expect(channels).toHaveLength(1));
        await push({ type: "error", code: "target_not_running", message: "The container isn't running." });
        expect(await screen.findByRole("heading", { name: `${web.name} isn't running` })).toBeInTheDocument();
        expect(screen.getByRole("status", { name: "Session status" })).toHaveTextContent(`Couldn't start a shell in ${web.name} · target_not_running`);
        expect(screen.getByRole("status", { name: "Session status" })).toHaveTextContent("0 of 4 sessions open");

        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: /START CONTAINER/ }));
        await waitFor(() =>
            expect(invoke).toHaveBeenCalledWith(
                "container_action",
                expect.objectContaining({ nodeId: web.nodeId, containerId: web.containerId, action: "start" }),
            ),
        );
        await waitFor(() => expect(channels).toHaveLength(2));
        await push({ type: "error", code: "target_not_running", message: "Still stopped." }, 1);

        await user.click(await screen.findByRole("button", { name: /VIEW LOGS/ }));
        await waitFor(() => expect(location()?.pathname).toBe("/logs"));
    });

    it("retries with a shell the image has when the default one is missing", async () => {
        renderPage();
        await waitFor(() => expect(channels).toHaveLength(1));
        await push({ type: "error", code: "shell_not_found", message: "No shell at /bin/sh in this container." });
        expect(await screen.findByRole("heading", { name: `No shell found in ${web.name}` })).toBeInTheDocument();
        const user = userEvent.setup();
        const retry = screen.getByRole("button", { name: /RETRY/ });
        expect(retry).toBeDisabled();
        await user.type(screen.getByRole("textbox", { name: "Shell to run" }), "/bin/ash");
        await user.click(retry);
        await waitFor(() => expect(invoke).toHaveBeenCalledWith("exec_open", expect.objectContaining({ shell: "/bin/ash" })));
    });

    it("explains a refused open and stops offering new shells at four", async () => {
        openAnswers = [() => Promise.reject("limit_reached")];
        renderPage();
        expect(await screen.findByRole("heading", { name: "Session limit reached" })).toBeInTheDocument();
        expect(screen.queryByRole("button", { name: /RETRY/ })).not.toBeInTheDocument();

        const user = userEvent.setup();
        for (let opened = 1; opened <= 4; opened += 1) {
            await user.click(screen.getByRole("button", { name: "New shell" }));
            await user.click(await screen.findByRole("option", { name: new RegExp(web.name) }));
            await waitFor(() => expect(channels).toHaveLength(opened + 1));
            await push({ type: "started", shell: "/bin/sh" }, opened);
        }
        expect(screen.getAllByRole("tab")).toHaveLength(5);
        expect(screen.getByRole("button", { name: "New shell" })).toHaveAttribute("aria-disabled", "true");
        expect(screen.getByRole("button", { name: /NEW SHELL/ })).toHaveAttribute("data-restricted", "true");
        expect(screen.getByRole("status", { name: "Session status" })).toHaveTextContent("4 active sessions");
        await user.click(screen.getByRole("button", { name: "New shell" }));
        expect(screen.queryByRole("option")).not.toBeInTheDocument();
        expect(within(screen.getByRole("tablist")).getAllByRole("tab")).toHaveLength(5);
    });
});
