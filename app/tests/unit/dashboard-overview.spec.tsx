import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { Route as HomeRoute } from "@/routes/_authenticated/index";

function renderHome() {
    const rootRoute = createRootRoute({});
    const homeRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/",
        component: HomeRoute.options.component,
    });
    const tasksRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/tasks",
        component: () => <h1>Containers destination</h1>,
    });
    const router = createRouter({
        routeTree: rootRoute.addChildren([homeRoute, tasksRoute]),
        history: createMemoryHistory({ initialEntries: ["/"] }),
    });
    return render(<RouterProvider router={router} />);
}

describe("home landing", () => {
    it("offers the read-only Containers destination without loading legacy dashboard resources", async () => {
        vi.clearAllMocks();
        renderHome();
        expect(await screen.findByRole("heading", { name: "Connected" })).toBeInTheDocument();
        expect(screen.getByText("Browse the configured node’s containers.")).toBeInTheDocument();
        expect(invoke).not.toHaveBeenCalled();
        await userEvent.setup().click(screen.getByRole("link", { name: "Open Containers" }));
        expect(await screen.findByRole("heading", { name: "Containers destination" })).toBeInTheDocument();
        expect(invoke).not.toHaveBeenCalled();
    });
});
