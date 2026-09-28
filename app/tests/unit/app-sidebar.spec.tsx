import { describe, expect, it } from "vitest";
import { render, screen, within } from "@testing-library/react";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, Outlet, RouterProvider } from "@tanstack/react-router";
import { AppSidebar, type SidebarBadges } from "@/components/shell/app-sidebar";
import { SidebarProvider } from "@/components/ui/sidebar";

function renderSidebar(path: string, badges: SidebarBadges = {}) {
    const rootRoute = createRootRoute({
        component: () => (
            <SidebarProvider>
                <AppSidebar badges={badges} agent={{ version: "0.1.0" }} />
                <Outlet />
            </SidebarProvider>
        ),
    });
    const routes = ["/", "/tasks", "/settings"].map((routePath) => createRoute({ getParentRoute: () => rootRoute, path: routePath, component: () => null }));
    const router = createRouter({ routeTree: rootRoute.addChildren(routes), history: createMemoryHistory({ initialEntries: [path] }) });
    render(<RouterProvider router={router} />);
}

describe("AppSidebar", () => {
    it("renders the eleven items in design order under three sections", async () => {
        renderSidebar("/");
        const items = await screen.findAllByRole("link");
        expect(items.map((item) => item.textContent)).toEqual([
            "Overview",
            "Containers",
            "Images",
            "Volumes",
            "Networks",
            "Stacks",
            "Nodes",
            "Services",
            "Exec",
            "Logs",
            "Settings",
        ]);
        for (const label of ["// INFRASTRUCTURE", "// ORCHESTRATION", "// SYSTEM"]) expect(screen.getByText(label)).toBeInTheDocument();
        expect(screen.getByText("AGENT v0.1.0")).toBeInTheDocument();
    });

    it("marks the active item from the router, with exact matching for Overview", async () => {
        renderSidebar("/settings");
        const settings = await screen.findByRole("link", { name: "Settings" });
        expect(settings).toHaveAttribute("data-status", "active");
        expect(screen.getByRole("link", { name: "Overview" })).not.toHaveAttribute("data-status", "active");
    });

    it("renders routes that do not exist yet as disabled", async () => {
        renderSidebar("/");
        expect(await screen.findByRole("link", { name: "Nodes" })).toHaveAttribute("aria-disabled", "true");
        expect(screen.getByRole("link", { name: "Stacks" })).not.toHaveAttribute("aria-disabled");
        expect(screen.getByRole("link", { name: "Networks" })).not.toHaveAttribute("aria-disabled");
        expect(screen.getByRole("link", { name: "Containers" })).not.toHaveAttribute("aria-disabled");
        expect(screen.getByRole("link", { name: "Images" })).not.toHaveAttribute("aria-disabled");
        expect(screen.getByRole("link", { name: "Volumes" })).not.toHaveAttribute("aria-disabled");
    });

    it("shows badges only when non-zero", async () => {
        renderSidebar("/", { containers: 24, nodesDown: 1 });
        expect(within(await screen.findByRole("link", { name: /Containers/ })).getByText("24")).toBeInTheDocument();
        expect(screen.getByTestId("nav-badge-nodes")).toHaveTextContent("1 DOWN");
    });

    it("hides badges at zero", async () => {
        renderSidebar("/", { containers: 0, nodesDown: 0 });
        await screen.findByRole("link", { name: "Containers" });
        expect(screen.queryByTestId("nav-badge-containers")).not.toBeInTheDocument();
        expect(screen.queryByTestId("nav-badge-nodes")).not.toBeInTheDocument();
    });
});
