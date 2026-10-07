import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, Outlet, RouterProvider } from "@tanstack/react-router";
import { Route as HubRoute } from "@/routes/_authenticated/settings/integrations/index";
import { Route as GitHubRoute } from "@/routes/_authenticated/settings/integrations/github";

/** The two pages on their own, without the signed-in shell, so every invoke would be theirs. */
function renderPages(path: string) {
    const rootRoute = createRootRoute({
        component: () => (
            <main>
                <Outlet />
            </main>
        ),
    });
    const page = (routePath: string, component: () => React.ReactNode) => createRoute({ getParentRoute: () => rootRoute, path: routePath, component });
    const routes = [
        page("/settings", () => <h1>Settings stand-in</h1>),
        page("/settings/integrations", HubRoute.options.component as () => React.ReactNode),
        page("/settings/integrations/github", GitHubRoute.options.component as () => React.ReactNode),
    ];
    const router = createRouter({ routeTree: rootRoute.addChildren(routes), history: createMemoryHistory({ initialEntries: [path] }) });
    render(<RouterProvider router={router} />);
    return router;
}

const crumbs = () =>
    within(screen.getByRole("navigation", { name: "Breadcrumb" }))
        .getAllByRole("listitem")
        .map((item) => item.textContent);

describe("Integrations", () => {
    beforeEach(() => vi.clearAllMocks());

    it("shows four not-connected cards and the Traefik skeleton, and only GitHub opens", async () => {
        const router = renderPages("/settings/integrations");
        expect(await screen.findByRole("heading", { name: "INTEGRATIONS" })).toBeInTheDocument();
        expect(screen.getByText("// manage external service connections")).toBeInTheDocument();
        expect(crumbs()).toEqual(["Settings", "Integrations"]);
        expect(screen.getByRole("link", { name: "Settings" })).toHaveAttribute("href", "/settings");

        for (const name of ["GitHub", "Slack", "Discord", "Webhooks"]) {
            const card = screen.getByLabelText(`${name} (not connected)`);
            expect(within(card).getByText("NOT CONNECTED")).toBeInTheDocument();
            expect(within(card).getByRole("heading", { name })).toBeInTheDocument();
        }
        const openable = screen.getAllByRole("link").filter((link) => /not connected/.test(link.getAttribute("aria-label") ?? ""));
        expect(openable.map((link) => link.getAttribute("aria-label"))).toEqual(["GitHub (not connected)"]);
        expect(within(screen.getByLabelText("Traefik (not configured)")).getByText("NOT CONFIGURED")).toBeInTheDocument();

        await userEvent.setup().click(screen.getByRole("link", { name: "GitHub (not connected)" }));
        await waitFor(() => expect(router.state.location.pathname).toBe("/settings/integrations/github"));
        expect(invoke).not.toHaveBeenCalled();
    });

    it("renders the GitHub page as headers over skeletons with no controls", async () => {
        const router = renderPages("/settings/integrations/github");
        expect(await screen.findByRole("heading", { name: "GITHUB INTEGRATION" })).toBeInTheDocument();
        expect(crumbs()).toEqual(["Settings", "Integrations", "GitHub"]);
        expect(screen.getByText("NOT CONNECTED")).toBeInTheDocument();
        expect(screen.getByRole("alert")).toHaveTextContent("GitHub connects when deployments arrive.");
        for (const title of ["OAuth Connection", "Container Registry (GHCR)", "OIDC Deploy Policies", "Danger Zone"]) {
            const section = screen.getByRole("region", { name: title });
            expect(within(section).getAllByTestId("skeleton-row").length).toBeGreaterThan(0);
        }
        expect(within(screen.getByRole("main")).queryAllByRole("button")).toHaveLength(0);

        await userEvent.setup().click(screen.getByRole("link", { name: "Integrations" }));
        await waitFor(() => expect(router.state.location.pathname).toBe("/settings/integrations"));
        expect(invoke).not.toHaveBeenCalled();
    });
});
