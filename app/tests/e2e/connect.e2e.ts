import { createServer } from "node:http";
import { once } from "node:events";
import { $, browser, expect } from "@wdio/globals";

// Only HTTP is faked: the built webview, Tauri commands, Rust client, and validation are real.
// Unknown endpoints fail so this fixture cannot silently stand in for a Docker Swarm.
describe("Connect", () => {
    let ownerExists = true;
    let invalidBootstrap = false;
    let origin: string;
    let port: number;
    let bootstrapRequests = 0;
    const logins: unknown[] = [];
    const unexpected: string[] = [];
    const controller = createServer(async (request, response) => {
        response.setHeader("Content-Type", "application/json");
        if (request.method === "GET" && request.url === "/api/v1/auth/bootstrap") {
            bootstrapRequests += 1;
            response.end(invalidBootstrap ? "{}" : JSON.stringify({ ownerExists }));
        } else if (request.method === "POST" && request.url === "/api/v1/auth/password/login") {
            let body = "";
            for await (const chunk of request) body += chunk.toString();
            logins.push(JSON.parse(body));
            response.writeHead(401).end(JSON.stringify({ error: "invalid_credentials", attemptsRemaining: 7 }));
        } else if (request.method === "GET" && request.url === "/health/ready") {
            response.end("{}");
        } else {
            unexpected.push(`${request.method} ${request.url}`);
            response.writeHead(500).end(JSON.stringify({ error: "unexpected_fixture_request" }));
        }
    });

    async function connect() {
        const previousRequests = bootstrapRequests;
        await $("#agent-url").setValue(origin);
        await $("#owner-email").click();
        await browser.waitUntil(() => bootstrapRequests > previousRequests);
        await expect($("#agent-url")).not.toHaveAttribute("readonly");
    }

    before(async () => {
        controller.listen(0, "127.0.0.1");
        await once(controller, "listening");
        const address = controller.address();
        if (!address || typeof address === "string") throw new Error("Expected a loopback TCP controller");
        port = address.port;
        origin = `http://127.0.0.1:${port}`;
    });

    beforeEach(async () => {
        ownerExists = true;
        invalidBootstrap = false;
        logins.length = 0;
        const loginUrl = new URL("/login", await browser.getUrl());
        loginUrl.searchParams.set("origin", "");
        // Start with an empty prefill so each test contacts only its explicit fixture.
        await browser.execute((url) => {
            window.history.replaceState(null, "", url);
            return null;
        }, loginUrl.href);
        await browser.refresh();
        await expect($("h1=KONTAINER")).toBeDisplayed();
        await expect($("#agent-url")).not.toHaveAttribute("readonly");
    });

    after(async () => {
        controller.closeAllConnections();
        await new Promise<void>((resolve, reject) => controller.close((error) => (error ? reject(error) : resolve())));
        expect(unexpected).toEqual([]);
    });

    it("shows sign in when the controller has an owner", async () => {
        await connect();
        await expect($("button=Sign in")).toBeDisplayed();
        await expect($("#setup-code")).not.toExist();
        await expect($("#agent-url")).toHaveValue(origin);
    });

    it("shows first-time setup when the controller has no owner", async () => {
        ownerExists = false;
        await connect();
        await expect($("button=Create owner account")).toBeDisplayed();
        await expect($("#setup-code")).toBeDisplayed();
        await expect($("#owner-name")).toBeDisplayed();
        await expect($("#owner-password-confirm")).toBeDisplayed();
    });

    it("validates owner fields before sending a registration", async () => {
        ownerExists = false;
        await connect();
        await $("#owner-email").setValue("owner");
        await $("#owner-password").setValue("short");
        await $("button=Create owner account").click();
        await expect($("#owner-email")).toHaveAttribute("aria-invalid", "true");
        await expect($("#owner-password")).toHaveAttribute("aria-invalid", "true");
        await expect($("#setup-code")).toHaveAttribute("aria-invalid", "true");
        await expect($("#owner-name")).toHaveAttribute("aria-invalid", "true");
        await expect($("#owner-password-confirm")).toHaveAttribute("aria-invalid", "true");
        expect(logins).toEqual([]);
    });

    it("requires HTTPS for remote controller URLs", async () => {
        await connect();
        const previousRequests = bootstrapRequests;
        await $("#agent-url").setValue("http://agent.example.com");
        await $("#owner-email").click();
        await expect($("#agent-url")).toHaveAttribute("aria-invalid", "true");
        await expect($("#agent-url-helper")).toHaveText("Use https:// (http:// only for localhost).");
        expect(bootstrapRequests).toBe(previousRequests);
        expect(logins).toEqual([]);
    });

    it("clears a rejected password and preserves the email and URL", async () => {
        await connect();
        await $("#owner-email").setValue("owner@example.com");
        await $("#owner-password").setValue("wrong horse battery");
        await $("button=Sign in").click();
        await expect($("#owner-password")).toHaveAttribute("aria-invalid", "true");
        await expect($("#owner-password")).toHaveValue("");
        await expect($("#owner-email")).toHaveValue("owner@example.com");
        await expect($("#agent-url")).toHaveValue(origin);
        await expect($("#owner-password-helper")).toHaveText("Email or password is incorrect. 7 attempts left before a 15-minute lock on this email.");
        expect(logins).toEqual([{ email: "owner@example.com", password: "wrong horse battery" }]);
    });

    it("explains when an endpoint is not a Kontainer controller", async () => {
        invalidBootstrap = true;
        await connect();
        await expect($("#agent-url-helper")).toHaveText("This URL answered, but not like a Kontainer agent.");
        await expect($("#agent-url")).toHaveValue(origin);
        expect(logins).toEqual([]);
    });

    it("retries a refused connection when the controller comes back", async () => {
        controller.closeAllConnections();
        await new Promise<void>((resolve, reject) => controller.close((error) => (error ? reject(error) : resolve())));
        try {
            await $("#agent-url").setValue(origin);
            await $("#owner-email").click();
            await expect($("button=TRY AGAIN")).toBeDisplayed();
            await expect($('[aria-label="Connection diagnostics"] [data-state="failed"]')).toHaveText(/TCP\s*✗/);
            await expect($("#agent-url")).toHaveValue(origin);
        } finally {
            controller.listen(port, "127.0.0.1");
            await once(controller, "listening");
        }
        await $("button=TRY AGAIN").click();
        await expect($("button=Sign in")).toBeDisplayed();
        await expect($("#agent-url")).not.toHaveAttribute("aria-invalid");
        expect(logins).toEqual([]);
    });
});
