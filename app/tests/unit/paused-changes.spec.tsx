import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Trash2 } from "lucide-react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ActionButton } from "@/components/kit/action-button";
import { ConfirmDialog } from "@/components/kit/confirm-dialog";
import { RowActions } from "@/components/data-table/row-actions";
import { ConnectivityContext, type ConnectivityContextValue } from "@/contexts/connectivity-context-value";

const offline: ConnectivityContextValue = { probe: { status: "unreachable" }, reachable: false, reconnectedAt: null, retry: () => {}, retrying: false };

function renderOffline(children: ReactNode, value: ConnectivityContextValue = offline) {
    return render(
        <TooltipProvider delay={0}>
            <ConnectivityContext value={value}>{children}</ConnectivityContext>
        </TooltipProvider>,
    );
}

describe("changes while the controller is unreachable", () => {
    it("restricts mutating buttons with the reason and leaves the rest alone", async () => {
        const onClick = vi.fn();
        renderOffline(
            <>
                <ActionButton mutates onClick={onClick}>
                    CREATE VOLUME
                </ActionButton>
                <ActionButton onClick={onClick}>REFRESH</ActionButton>
            </>,
        );
        const create = screen.getByRole("button", { name: /CREATE VOLUME/ });
        expect(create).toHaveAttribute("aria-disabled", "true");
        const user = userEvent.setup();
        await user.hover(create);
        expect(await screen.findByText("Changes are paused while offline")).toBeInTheDocument();
        await user.click(create);
        expect(onClick).not.toHaveBeenCalled();
        await user.click(screen.getByRole("button", { name: "REFRESH" }));
        expect(onClick).toHaveBeenCalledOnce();
    });

    it("keeps a confirm dialog from submitting, even with Enter", async () => {
        const onConfirm = vi.fn();
        renderOffline(<ConfirmDialog open onOpenChange={() => {}} title='Remove web?' confirmationText='web' confirmLabel='REMOVE' onConfirm={onConfirm} />);
        const user = userEvent.setup();
        await user.type(screen.getByLabelText("Type web to confirm"), "web{Enter}");
        expect(screen.getByRole("button", { name: /REMOVE/ })).toHaveAttribute("aria-disabled", "true");
        expect(onConfirm).not.toHaveBeenCalled();
    });

    it("disables mutating row actions and says why in the menu, leaving navigation alone", async () => {
        const remove = vi.fn();
        const show = vi.fn();
        renderOffline(
            <RowActions
                label='toolbox'
                actions={[{ label: "Delete", icon: Trash2, onClick: remove, mutates: true }]}
                menu={[
                    { label: "Show services", onClick: show },
                    { label: "Remove", onClick: remove, mutates: true },
                ]}
            />,
        );
        expect(screen.getByRole("button", { name: "Delete" })).toHaveAttribute("aria-disabled", "true");
        const user = userEvent.setup();
        await user.click(screen.getByRole("button", { name: "More actions for toolbox" }));
        expect(await screen.findByText("Changes are paused while offline")).toBeInTheDocument();
        expect(screen.getByRole("menuitem", { name: "Remove" })).toHaveAttribute("aria-disabled", "true");
        await user.click(screen.getByRole("menuitem", { name: "Show services" }));
        expect(show).toHaveBeenCalledOnce();
        expect(remove).not.toHaveBeenCalled();
    });

    it("changes nothing while the controller answers", () => {
        renderOffline(<ActionButton mutates>CREATE VOLUME</ActionButton>, { ...offline, reachable: true, probe: { status: "ready" } });
        expect(screen.getByRole("button", { name: /CREATE VOLUME/ })).not.toHaveAttribute("aria-disabled");
    });
});
