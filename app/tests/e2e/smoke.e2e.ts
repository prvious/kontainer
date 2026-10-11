import { $, expect } from "@wdio/globals";

describe("Kontainer desktop", () => {
    // ponytail: assumes a clean e2e profile; future login tests must sign out and remove their saved fixture connection.
    it("opens on the Connect card when no agent is saved", async () => {
        await expect($("h1=KONTAINER")).toBeDisplayed();

        const agentUrl = $('input[placeholder="https://agent.example.com"]');
        await expect(agentUrl).not.toHaveAttribute("readonly");
        await agentUrl.setValue("https://agent.example.com");
        await expect(agentUrl).toHaveValue("https://agent.example.com");
    });
});
