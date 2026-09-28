import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ConfirmDialog } from "@/components/kit/confirm-dialog";

function renderDialog(props: Partial<Parameters<typeof ConfirmDialog>[0]> = {}) {
    const onConfirm = vi.fn();
    const onOpenChange = vi.fn();
    render(
        <ConfirmDialog
            open
            onOpenChange={onOpenChange}
            title='Delete image nginx:1.25-alpine?'
            description='The image is removed from every node that has it.'
            impact='In use by 2 containers: nginx-proxy-01, nginx-proxy-02'
            confirmationText='nginx:1.25-alpine'
            confirmLabel='DELETE IMAGE'
            onConfirm={onConfirm}
            {...props}
        />,
    );
    return { onConfirm, onOpenChange };
}

describe("ConfirmDialog", () => {
    it("enables the confirm button only on an exact typed match", async () => {
        const user = userEvent.setup();
        const { onConfirm } = renderDialog();
        const confirm = screen.getByRole("button", { name: /DELETE IMAGE/ });
        expect(confirm).toBeDisabled();
        expect(screen.getByText("In use by 2 containers: nginx-proxy-01, nginx-proxy-02")).toBeInTheDocument();

        const input = screen.getByLabelText("Type nginx:1.25-alpine to confirm");
        await user.type(input, "nginx:1.25-alp");
        expect(confirm).toBeDisabled();
        await user.type(input, "ine ");
        expect(confirm).toBeDisabled();
        await user.type(input, "{Backspace}");
        expect(confirm).toBeEnabled();

        await user.click(confirm);
        expect(onConfirm).toHaveBeenCalledTimes(1);
    });

    it("closes on Esc and Cancel without calling the action", async () => {
        const user = userEvent.setup();
        const { onConfirm, onOpenChange } = renderDialog();
        await user.keyboard("{Escape}");
        expect(onOpenChange).toHaveBeenCalledWith(false, expect.anything());
        await user.click(screen.getByRole("button", { name: "CANCEL" }));
        expect(onOpenChange).toHaveBeenLastCalledWith(false);
        expect(onConfirm).not.toHaveBeenCalled();
    });

    it("confirms at once without typed confirmation, and blocks a second submit while pending", async () => {
        const user = userEvent.setup();
        const { onConfirm } = renderDialog({ confirmationText: undefined, pending: true });
        const confirm = screen.getByRole("button", { name: /DELETE IMAGE/ });
        expect(confirm).toHaveAttribute("aria-busy", "true");
        await user.click(confirm);
        expect(onConfirm).not.toHaveBeenCalled();
    });
});
