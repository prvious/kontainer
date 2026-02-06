import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { AuthProvider } from "@/contexts/auth-context";
import { LoginScreen } from "@/components/login-screen";

describe("LoginScreen", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    function renderLoginScreen() {
        return render(
            <AuthProvider>
                <LoginScreen />
            </AuthProvider>,
        );
    }

    it("renders the sign in heading", () => {
        renderLoginScreen();
        expect(screen.getByText("Sign in to Devoper")).toBeInTheDocument();
    });

    it("renders the description", () => {
        renderLoginScreen();
        expect(screen.getByText("Connect to your agent to get started.")).toBeInTheDocument();
    });

    it("renders the agent URL input with default value", () => {
        renderLoginScreen();
        const input = screen.getByLabelText("Agent URL");
        expect(input).toBeInTheDocument();
        expect(input).toHaveValue("http://localhost:8080");
    });

    it("renders the sign in button", () => {
        renderLoginScreen();
        expect(screen.getByRole("button", { name: /sign in with github/i })).toBeInTheDocument();
    });

    it("calls openUrl with correct auth URL on submit", async () => {
        renderLoginScreen();
        const user = userEvent.setup();

        const button = screen.getByRole("button", {
            name: /sign in with github/i,
        });
        await user.click(button);

        expect(openUrl).toHaveBeenCalledWith("http://localhost:8080/auth/github");
    });

    it("allows changing the agent URL", async () => {
        renderLoginScreen();
        const user = userEvent.setup();

        const input = screen.getByLabelText("Agent URL");
        await user.clear(input);
        await user.type(input, "http://myserver:9090");

        expect(input).toHaveValue("http://myserver:9090");
    });

    it("strips trailing slashes from agent URL before login", async () => {
        renderLoginScreen();
        const user = userEvent.setup();

        const input = screen.getByLabelText("Agent URL");
        await user.clear(input);
        await user.type(input, "http://myserver:9090///");

        const button = screen.getByRole("button", {
            name: /sign in with github/i,
        });
        await user.click(button);

        expect(openUrl).toHaveBeenCalledWith("http://myserver:9090/auth/github");
    });
});
