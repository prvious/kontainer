import { $, expect } from "@wdio/globals";

describe("Kontainer desktop", () => {
    // ponytail: assumes a clean e2e profile; the first test that signs in must sign out in `after`.
    it("opens on the Connect card when no agent is saved", async () => {
        await expect($("h1=KONTAINER")).toBeDisplayed();

        const agentUrl = $('input[placeholder="https://agent.example.com"]');
        await agentUrl.setValue("https://agent.example.com");
        await expect(agentUrl).toHaveValue("https://agent.example.com");
    });
});
