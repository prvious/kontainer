import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { SetupWizard } from "@/components/setup-wizard";

describe("SetupWizard", () => {
    const defaultProps = {
        agentUrl: "http://localhost:3000",
        onComplete: vi.fn(),
        onBack: vi.fn(),
    };

    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("renders token input on initial step", () => {
        render(<SetupWizard {...defaultProps} />);
        expect(screen.getByText("SETUP TOKEN")).toBeInTheDocument();
        expect(screen.getByPlaceholderText("kst_live_29f0-8d0z-1d7c")).toBeInTheDocument();
        expect(screen.getByRole("button", { name: /validate token & start/i })).toBeInTheDocument();
    });

    it("renders cancel button that calls onBack", async () => {
        render(<SetupWizard {...defaultProps} />);
        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: /cancel/i }));
        expect(defaultProps.onBack).toHaveBeenCalled();
    });

    it("shows error on invalid token", async () => {
        vi.mocked(invoke).mockRejectedValueOnce("Invalid setup token");

        render(<SetupWizard {...defaultProps} />);
        const user = userEvent.setup();

        const input = screen.getByPlaceholderText("kst_live_29f0-8d0z-1d7c");
        await user.type(input, "wrong_token");
        await user.click(screen.getByRole("button", { name: /validate token & start/i }));

        await act(async () => {});
        expect(screen.getByText("Invalid setup token")).toBeInTheDocument();
    });

    it("calls setup_github_connect on valid token with pending status", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "setup_verify_token") return { step: "pending" };
            if (cmd === "setup_github_connect") return { setupUrl: "http://localhost:3000/setup/github/start?token=abc" };
            return undefined;
        });

        render(<SetupWizard {...defaultProps} />);
        const user = userEvent.setup();

        const input = screen.getByPlaceholderText("kst_live_29f0-8d0z-1d7c");
        await user.type(input, "ktsetup_abc123");
        await user.click(screen.getByRole("button", { name: /validate token & start/i }));

        await act(async () => {});
        expect(invoke).toHaveBeenCalledWith("setup_github_connect", {
            url: "http://localhost:3000",
            token: "ktsetup_abc123",
            org: null,
        });
    });

    it("opens the setup URL after github connect", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "setup_verify_token") return { step: "pending" };
            if (cmd === "setup_github_connect") return { setupUrl: "http://localhost:3000/setup/github/start?token=abc" };
            return undefined;
        });

        render(<SetupWizard {...defaultProps} />);
        const user = userEvent.setup();

        const input = screen.getByPlaceholderText("kst_live_29f0-8d0z-1d7c");
        await user.type(input, "ktsetup_abc123");
        await user.click(screen.getByRole("button", { name: /validate token & start/i }));

        await act(async () => {});
        expect(openUrl).toHaveBeenCalledWith("http://localhost:3000/setup/github/start?token=abc");
    });

    it("passes null org to setup_github_connect", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "setup_verify_token") return { step: "pending" };
            if (cmd === "setup_github_connect") return { setupUrl: "http://localhost:3000/setup/github/start?token=abc" };
            return undefined;
        });

        render(<SetupWizard {...defaultProps} />);
        const user = userEvent.setup();

        const input = screen.getByPlaceholderText("kst_live_29f0-8d0z-1d7c");
        await user.type(input, "ktsetup_abc123");
        await user.click(screen.getByRole("button", { name: /validate token & start/i }));

        await act(async () => {});
        expect(invoke).toHaveBeenCalledWith("setup_github_connect", {
            url: "http://localhost:3000",
            token: "ktsetup_abc123",
            org: null,
        });
    });

    it("calls setup_complete and onComplete when github already connected", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "setup_verify_token") return { step: "github_connected" };
            if (cmd === "setup_complete") return undefined;
            return undefined;
        });

        render(<SetupWizard {...defaultProps} />);
        const user = userEvent.setup();

        const input = screen.getByPlaceholderText("kst_live_29f0-8d0z-1d7c");
        await user.type(input, "ktsetup_abc123");
        await user.click(screen.getByRole("button", { name: /validate token & start/i }));

        await act(async () => {});
        expect(invoke).toHaveBeenCalledWith("setup_complete", {
            url: "http://localhost:3000",
            token: "ktsetup_abc123",
        });
        expect(defaultProps.onComplete).toHaveBeenCalled();
    });

    it("calls setup_complete with correct args", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "setup_verify_token") return { step: "github_connected" };
            if (cmd === "setup_complete") return undefined;
            return undefined;
        });

        render(<SetupWizard {...defaultProps} />);
        const user = userEvent.setup();

        const input = screen.getByPlaceholderText("kst_live_29f0-8d0z-1d7c");
        await user.type(input, "ktsetup_abc123");
        await user.click(screen.getByRole("button", { name: /validate token & start/i }));

        await act(async () => {});
        expect(invoke).toHaveBeenCalledWith("setup_complete", {
            url: "http://localhost:3000",
            token: "ktsetup_abc123",
        });
    });

    it("displays agent URL in footer", () => {
        render(<SetupWizard {...defaultProps} />);
        expect(screen.getByText(/http:\/\/localhost:3000/)).toBeInTheDocument();
    });

    it("renders FIRST-TIME SETUP badge", () => {
        render(<SetupWizard {...defaultProps} />);
        expect(screen.getByText("FIRST-TIME SETUP")).toBeInTheDocument();
    });

    it("renders the heading", () => {
        render(<SetupWizard {...defaultProps} />);
        expect(screen.getByText("Create your GitHub App manifest")).toBeInTheDocument();
    });

    it("does not render a dead appName input", () => {
        render(<SetupWizard {...defaultProps} />);
        expect(screen.queryByText("GITHUB APP NAME")).not.toBeInTheDocument();
        expect(screen.queryByDisplayValue("kontainer-prod-app")).not.toBeInTheDocument();
    });

    it("disables Validate Token & Start button when token is empty", () => {
        render(<SetupWizard {...defaultProps} />);
        const button = screen.getByRole("button", { name: /validate token & start/i });
        expect(button).toBeDisabled();
    });

    it("shows error after consecutive polling failures", async () => {
        vi.useFakeTimers({ shouldAdvanceTime: true });
        let callCount = 0;

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "setup_verify_token") {
                callCount++;
                // First call succeeds (returns pending so we start GitHub connect flow)
                if (callCount === 1) return { step: "pending" };
                // All subsequent polling calls fail
                throw "Connection lost";
            }
            if (cmd === "setup_github_connect") {
                return { setupUrl: "http://localhost:3000/setup/github/start?token=abc" };
            }
            return undefined;
        });

        const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
        render(<SetupWizard {...defaultProps} />);

        const input = screen.getByPlaceholderText("kst_live_29f0-8d0z-1d7c");
        await user.type(input, "ktsetup_abc123");
        await user.click(screen.getByRole("button", { name: /validate token & start/i }));

        // Wait for initial flow to complete (verify token + github connect + openUrl)
        await act(async () => {});

        // Advance through 5 polling intervals (5 * 2000ms = 10000ms)
        for (let i = 0; i < 5; i++) {
            await act(async () => {
                vi.advanceTimersByTime(2000);
            });
            // Let microtasks (the rejected promise in setInterval callback) resolve
            await act(async () => {});
        }

        // After 5 consecutive failures, error should be shown
        expect(screen.getByText("Connection lost")).toBeInTheDocument();
        // Loading state should be cleared (button re-enabled)
        expect(screen.getByRole("button", { name: /validate token & start/i })).not.toBeDisabled();

        vi.useRealTimers();
    });
});
