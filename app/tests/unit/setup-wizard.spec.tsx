import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
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
        expect(screen.getByLabelText("Setup Token")).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Verify" })).toBeInTheDocument();
    });

    it("renders back button that calls onBack", async () => {
        render(<SetupWizard {...defaultProps} />);
        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: /back/i }));
        expect(defaultProps.onBack).toHaveBeenCalled();
    });

    it("shows error on invalid token", async () => {
        vi.mocked(invoke).mockRejectedValueOnce("Invalid setup token");

        render(<SetupWizard {...defaultProps} />);
        const user = userEvent.setup();

        const input = screen.getByLabelText("Setup Token");
        await user.type(input, "wrong_token");
        await user.click(screen.getByRole("button", { name: "Verify" }));

        await act(async () => {});
        expect(screen.getByText("Invalid setup token")).toBeInTheDocument();
    });

    it("advances to github step on valid token with pending status", async () => {
        vi.mocked(invoke).mockResolvedValueOnce({ step: "pending" });

        render(<SetupWizard {...defaultProps} />);
        const user = userEvent.setup();

        const input = screen.getByLabelText("Setup Token");
        await user.type(input, "ktsetup_abc123");
        await user.click(screen.getByRole("button", { name: "Verify" }));

        await act(async () => {});
        expect(screen.getByRole("button", { name: /connect github/i })).toBeInTheDocument();
    });

    it("passes org to setup_github_connect when provided", async () => {
        vi.mocked(invoke)
            .mockResolvedValueOnce({ step: "pending" }) // verify token
            .mockResolvedValueOnce({ setupUrl: "http://localhost:3000/setup/github/start?token=abc" }); // github connect

        render(<SetupWizard {...defaultProps} />);
        const user = userEvent.setup();

        // Enter token and verify
        const input = screen.getByLabelText("Setup Token");
        await user.type(input, "ktsetup_abc123");
        await user.click(screen.getByRole("button", { name: "Verify" }));
        await act(async () => {});

        // Enter org and connect
        const orgInput = screen.getByLabelText(/organization/i);
        await user.type(orgInput, "my-org");
        await user.click(screen.getByRole("button", { name: /connect github/i }));
        await act(async () => {});

        expect(invoke).toHaveBeenCalledWith("setup_github_connect", {
            url: "http://localhost:3000",
            token: "ktsetup_abc123",
            org: "my-org",
        });
    });

    it("passes null org when field is empty", async () => {
        vi.mocked(invoke)
            .mockResolvedValueOnce({ step: "pending" }) // verify token
            .mockResolvedValueOnce({ setupUrl: "http://localhost:3000/setup/github/start?token=abc" }); // github connect

        render(<SetupWizard {...defaultProps} />);
        const user = userEvent.setup();

        const input = screen.getByLabelText("Setup Token");
        await user.type(input, "ktsetup_abc123");
        await user.click(screen.getByRole("button", { name: "Verify" }));
        await act(async () => {});

        await user.click(screen.getByRole("button", { name: /connect github/i }));
        await act(async () => {});

        expect(invoke).toHaveBeenCalledWith("setup_github_connect", {
            url: "http://localhost:3000",
            token: "ktsetup_abc123",
            org: null,
        });
    });

    it("resumes at confirm step if github already connected", async () => {
        vi.mocked(invoke).mockResolvedValueOnce({ step: "github_connected" });

        render(<SetupWizard {...defaultProps} />);
        const user = userEvent.setup();

        const input = screen.getByLabelText("Setup Token");
        await user.type(input, "ktsetup_abc123");
        await user.click(screen.getByRole("button", { name: "Verify" }));

        await act(async () => {});
        expect(screen.getByRole("button", { name: /complete setup/i })).toBeInTheDocument();
    });

    it("calls setup_complete and onComplete on final step", async () => {
        vi.mocked(invoke)
            .mockResolvedValueOnce({ step: "github_connected" }) // verify token
            .mockResolvedValueOnce(undefined); // setup_complete

        render(<SetupWizard {...defaultProps} />);
        const user = userEvent.setup();

        // Enter token and verify
        const input = screen.getByLabelText("Setup Token");
        await user.type(input, "ktsetup_abc123");
        await user.click(screen.getByRole("button", { name: "Verify" }));
        await act(async () => {});

        // Complete setup
        await user.click(screen.getByRole("button", { name: /complete setup/i }));
        await act(async () => {});

        expect(invoke).toHaveBeenCalledWith("setup_complete", {
            url: "http://localhost:3000",
            token: "ktsetup_abc123",
        });
        expect(defaultProps.onComplete).toHaveBeenCalled();
    });

    it("displays agent URL", () => {
        render(<SetupWizard {...defaultProps} />);
        expect(screen.getByText("http://localhost:3000")).toBeInTheDocument();
    });
});
