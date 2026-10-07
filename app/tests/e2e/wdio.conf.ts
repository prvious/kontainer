import path from "node:path";
import { fileURLToPath } from "node:url";

// The debug binary `pnpm e2e:build` produces: the `e2e` feature, its own identifier and vault
// service, and the bundled frontend (ADR 0044).
const binary = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    "../../src-tauri/target/debug",
    process.platform === "win32" ? "kontainer-desktop.exe" : "kontainer-desktop",
);

export const config: WebdriverIO.Config = {
    runner: "local",
    specs: ["./**/*.e2e.ts"],
    maxInstances: 1,
    services: [["@wdio/tauri-service", { driverProvider: "embedded", appBinaryPath: binary }]],
    capabilities: [{ browserName: "tauri" }],
    framework: "mocha",
    reporters: ["spec"],
    mochaOpts: { ui: "bdd", timeout: 60_000 },
    logLevel: "warn",
    waitforTimeout: 15_000,
    // Pin the main window. Without an explicit switch, the service re-checks window focus before
    // every command through `tauri-plugin-wdio`, which this app doesn't embed, and waits 5 s each
    // time for an invoke that never comes.
    before: async () => {
        await browser.switchToWindow("main");
    },
};
