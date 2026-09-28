import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { AuthProvider } from "@/contexts/auth-context";
import { ConnectCard } from "@/components/connect/connect-card";
import { probeAll } from "@/hooks/session/use-session-connections";
import type { SavedConnection } from "@/types/session";

const saved: SavedConnection[] = [
    { id: "c1", origin: "https://agent.kontainer.dev/", label: "Production Cluster", lastUsedAtUnixMs: Date.now() - 3_600_000, lastSummary: null },
    { id: "c2", origin: "https://staging.kontainer.dev/", label: "Staging", lastUsedAtUnixMs: Date.now() - 7_200_000, lastSummary: null },
];

type Handler = (args: Record<string, unknown>) => unknown;

function mockCommands(overrides: Record<string, Handler> = {}) {
    vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
        if (command in overrides) return overrides[command]((args ?? {}) as Record<string, unknown>);
        switch (command) {
            case "session_restore":
                return null;
            case "session_connections":
                return [];
            case "session_bootstrap":
                return { ownerExists: true };
            case "connection_probe":
                return { status: "ready" };
            default:
                return null;
        }
    });
}

function renderCard(initialOrigin?: string) {
    return render(
        <AuthProvider>
            <ConnectCard initialOrigin={initialOrigin} />
        </AuthProvider>,
    );
}

describe("Connect card", () => {
    beforeEach(() => vi.clearAllMocks());

    it("checks the prefilled URL and switches to Create owner when the agent has none", async () => {
        mockCommands({ session_bootstrap: () => ({ ownerExists: false }) });
        renderCard();
        expect(await screen.findByText("First-time setup")).toBeInTheDocument();
        expect(invoke).toHaveBeenCalledWith("session_bootstrap", { origin: "http://127.0.0.1:5150" });
        expect(screen.getByLabelText("Name")).toBeInTheDocument();
        expect(screen.getByLabelText("Confirm password")).toBeInTheDocument();
        expect(screen.getByRole("button", { name: /Create owner account/ })).toBeInTheDocument();
        expect(screen.getByText("This account becomes the Owner. Registration closes once it exists.")).toBeInTheDocument();
        expect(screen.queryByText("Saved connections")).not.toBeInTheDocument();
    });

    it("rechecks on URL blur and shows Sign in for an agent with an owner", async () => {
        const user = userEvent.setup();
        mockCommands({ session_bootstrap: ({ origin }) => ({ ownerExists: origin !== "http://127.0.0.1:5150" }) });
        renderCard();
        await screen.findByText("First-time setup");
        const url = screen.getByLabelText("Agent URL");
        await user.clear(url);
        await user.type(url, "https://agent.kontainer.dev");
        await user.tab();
        await waitFor(() => expect(screen.getByRole("button", { name: /Sign in/ })).toBeInTheDocument());
        expect(screen.queryByText("First-time setup")).not.toBeInTheDocument();
    });

    it("validates on blur and on submit without calling the agent", async () => {
        const user = userEvent.setup();
        mockCommands({ session_bootstrap: () => ({ ownerExists: false }) });
        renderCard();
        await screen.findByText("First-time setup");
        await user.type(screen.getByLabelText("Email"), "owner");
        await user.tab();
        expect(screen.getByText("Enter an email like owner@example.com.")).toBeInTheDocument();
        await user.type(screen.getByLabelText("Password"), "short");
        await user.type(screen.getByLabelText("Confirm password"), "different");
        await user.click(screen.getByRole("button", { name: /Create owner account/ }));
        expect(screen.getByText("Enter your name.")).toBeInTheDocument();
        expect(screen.getByText("Use at least 12 characters.")).toBeInTheDocument();
        expect(screen.getByText("The passwords don't match.")).toBeInTheDocument();
        expect(screen.getByLabelText("Password")).toHaveAttribute("aria-invalid", "true");
        expect(invoke).not.toHaveBeenCalledWith("session_register", expect.anything());
    });

    it("lands server field errors on their fields", async () => {
        const user = userEvent.setup();
        mockCommands({
            session_bootstrap: () => ({ ownerExists: false }),
            session_register: () => Promise.reject({ code: "validation_failed", fields: { email: "invalid", displayName: "too_long" } }),
        });
        renderCard();
        await screen.findByText("First-time setup");
        await user.type(screen.getByLabelText("Name"), "Ada");
        await user.type(screen.getByLabelText("Email"), "ada@example.com");
        await user.type(screen.getByLabelText("Password"), "correct horse battery");
        await user.type(screen.getByLabelText("Confirm password"), "correct horse battery");
        await user.click(screen.getByRole("button", { name: /Create owner account/ }));
        expect(await screen.findByText("Use at most 80 characters.")).toBeInTheDocument();
        expect(invoke).toHaveBeenCalledWith("session_register", {
            origin: "http://127.0.0.1:5150",
            displayName: "Ada",
            email: "ada@example.com",
            password: "correct horse battery",
            label: null,
        });
    });

    it("02.S2: keeps the typed URL, names the failed step, and offers TRY AGAIN", async () => {
        const user = userEvent.setup();
        let attempts = 0;
        mockCommands({
            session_bootstrap: () => {
                attempts += 1;
                return Promise.reject({ code: "controller_unavailable" });
            },
            connection_probe: () => ({ status: "unreachable", diagnostic: "tcp" }),
        });
        renderCard("https://agent.kontainer.dev:9443");
        expect(
            await screen.findByText("Connection refused — nothing is listening on :9443. Check the port, or the agent's firewall rules."),
        ).toBeInTheDocument();
        expect(screen.getByLabelText("Agent URL")).toHaveValue("https://agent.kontainer.dev:9443");
        expect(screen.getByLabelText("Agent URL")).toHaveAttribute("aria-invalid", "true");
        const diagnostics = screen.getByLabelText("Connection diagnostics");
        expect(within(diagnostics).getByText(/DNS/)).toHaveAttribute("data-state", "ok");
        expect(within(diagnostics).getByText(/TCP/)).toHaveAttribute("data-state", "failed");
        expect(within(diagnostics).getByText(/TLS/)).toHaveAttribute("data-state", "skipped");
        await user.click(screen.getByRole("button", { name: /TRY AGAIN/ }));
        await waitFor(() => expect(attempts).toBe(2));
        expect(invoke).not.toHaveBeenCalledWith("session_login", expect.anything());
    });

    it("locks the URL and shows CONNECTING… while the agent is checked", async () => {
        mockCommands({ session_bootstrap: () => new Promise(() => {}) });
        renderCard("https://agent.kontainer.dev");
        const button = await screen.findByRole("button", { name: /CONNECTING…/ });
        expect(button).toHaveAttribute("aria-busy", "true");
        expect(screen.getByLabelText("Agent URL")).toHaveAttribute("readonly");
    });

    it("02.S7: marks and clears the password, keeps email and URL, and counts attempts down", async () => {
        const user = userEvent.setup();
        mockCommands({ session_login: () => Promise.reject({ code: "invalid_credentials", attemptsRemaining: 7 }) });
        renderCard("https://agent.kontainer.dev");
        await screen.findByRole("button", { name: /Sign in/ });
        await user.type(screen.getByLabelText("Email"), "owner@example.com");
        await user.type(screen.getByLabelText("Password"), "wrong horse battery");
        await user.click(screen.getByRole("button", { name: /Sign in/ }));
        expect(await screen.findByText("Email or password is incorrect. 7 attempts left before a 15-minute lock on this email.")).toBeInTheDocument();
        expect(screen.getByLabelText("Password")).toHaveValue("");
        expect(screen.getByLabelText("Password")).toHaveAttribute("aria-invalid", "true");
        expect(screen.getByLabelText("Email")).toHaveValue("owner@example.com");
        expect(screen.getByLabelText("Agent URL")).toHaveValue("https://agent.kontainer.dev");
    });

    it("shows the lock with the retry time after ten failures", async () => {
        const user = userEvent.setup();
        mockCommands({ session_login: () => Promise.reject({ code: "login_locked", retryAfterSeconds: 840 }) });
        renderCard("https://agent.kontainer.dev");
        await screen.findByRole("button", { name: /Sign in/ });
        await user.type(screen.getByLabelText("Email"), "owner@example.com");
        await user.type(screen.getByLabelText("Password"), "wrong horse battery");
        await user.click(screen.getByRole("button", { name: /Sign in/ }));
        expect(await screen.findByText(/locked for 14 more minutes \(until \d\d:\d\d\)/)).toBeInTheDocument();
    });

    it("fills the URL from a saved connection and focuses the email", async () => {
        const user = userEvent.setup();
        mockCommands({ session_connections: () => saved });
        renderCard();
        // The most recent connection prefills the URL.
        await waitFor(() => expect(screen.getByLabelText("Agent URL")).toHaveValue("https://agent.kontainer.dev/"));
        expect(screen.getByText("2 AGENTS")).toBeInTheDocument();
        await user.click(screen.getByRole("button", { name: /Staging/ }));
        expect(screen.getByLabelText("Agent URL")).toHaveValue("https://staging.kontainer.dev/");
        expect(screen.getByLabelText("Email")).toHaveFocus();
        await waitFor(() => expect(invoke).toHaveBeenCalledWith("session_bootstrap", { origin: "https://staging.kontainer.dev/" }));
    });

    it("colours saved-connection dots from one probe each", async () => {
        mockCommands({
            session_connections: () => saved,
            connection_probe: ({ origin }) =>
                origin === "https://staging.kontainer.dev/" ? { status: "unreachable", diagnostic: "dns" } : { status: "ready" },
        });
        renderCard();
        await waitFor(() => expect(screen.getByTestId("probe-c2")).toHaveAttribute("data-tone", "destructive"));
        expect(screen.getByTestId("probe-c1")).toHaveAttribute("data-tone", "success");
    });
});

describe("probeAll", () => {
    it("keeps at most three probes in flight and never lets one failure stop the rest", async () => {
        let inFlight = 0;
        let peak = 0;
        vi.mocked(invoke).mockImplementation(async (_command: string, args?: unknown) => {
            inFlight += 1;
            peak = Math.max(peak, inFlight);
            await new Promise((resolve) => setTimeout(resolve, 5));
            inFlight -= 1;
            if ((args as { origin: string }).origin.includes("bad")) throw "invalid_origin";
            return { status: "ready" };
        });
        const origins = ["https://a.dev/", "https://bad.dev/", "https://c.dev/", "https://d.dev/", "https://e.dev/", "https://f.dev/", "https://g.dev/"];
        const results = await probeAll(origins);
        expect(peak).toBe(3);
        expect(Object.keys(results)).toHaveLength(7);
        expect(results["https://bad.dev/"]).toEqual({ status: "unreachable" });
        expect(results["https://g.dev/"]).toEqual({ status: "ready" });
    });
});
