import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { Route as ExecRoute } from "@/routes/_authenticated/exec/index";

describe("unported exec route", () => {
    it("is inert and does not start a direct-agent inventory or terminal", async () => {
        vi.clearAllMocks();
        const rootRoute = createRootRoute({});
        const execRoute = createRoute({
            getParentRoute: () => rootRoute,
            path: "/exec",
            component: ExecRoute.options.component,
        });
        const router = createRouter({
            routeTree: rootRoute.addChildren([execRoute]),
            history: createMemoryHistory({ initialEntries: ["/exec"] }),
        });
        render(<RouterProvider router={router} />);
        expect(await screen.findByText("Exec will be available after its native client flow is ported.")).toBeInTheDocument();
        expect(invoke).not.toHaveBeenCalled();
    });
});
