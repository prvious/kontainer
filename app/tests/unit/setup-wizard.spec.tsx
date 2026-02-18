import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { SetupWizard } from "@/components/setup-wizard";

describe("SetupWizard", () => {
    const defaultProps = {
        agentUrl: "http://localhost:8080",
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
            url: "http://localhost:8080",
            token: "ktsetup_abc123",
        });
        expect(defaultProps.onComplete).toHaveBeenCalled();
    });

    it("displays agent URL", () => {
        render(<SetupWizard {...defaultProps} />);
        expect(screen.getByText("http://localhost:8080")).toBeInTheDocument();
    });
});
