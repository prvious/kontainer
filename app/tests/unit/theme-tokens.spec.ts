import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

// Every `--*` colour variable in docs/design.pen (GetVariables, `mode` axis).
const DESIGN_COLOR_TOKENS = [
    "accent",
    "accent-foreground",
    "background",
    "border",
    "card",
    "card-foreground",
    "chart-1",
    "chart-2",
    "chart-3",
    "chart-4",
    "chart-5",
    "destructive",
    "destructive-faint",
    "destructive-foreground",
    "destructive-soft",
    "foreground",
    "foreground-faint",
    "input",
    "muted",
    "muted-foreground",
    "popover",
    "popover-foreground",
    "primary",
    "primary-deep",
    "primary-faint",
    "primary-foreground",
    "primary-soft",
    "ring",
    "scrim",
    "secondary",
    "secondary-foreground",
    "sidebar",
    "sidebar-accent",
    "sidebar-accent-foreground",
    "sidebar-border",
    "sidebar-foreground",
    "sidebar-primary",
    "sidebar-primary-foreground",
    "sidebar-ring",
    "statusbar",
    "success",
    "success-foreground",
    "success-soft",
    "titlebar",
    "warning",
    "warning-foreground",
    "warning-soft",
];

const css = readFileSync(path.resolve(__dirname, "../../src/app.css"), "utf8");

function block(selector: string): string {
    const start = css.indexOf(`${selector} {`);
    expect(start, `${selector} block`).toBeGreaterThanOrEqual(0);
    return css.slice(start, css.indexOf("}", start));
}

describe("design tokens in app.css", () => {
    it.each([":root", ".dark"])("%s defines every design colour", (selector) => {
        const body = block(selector);
        const missing = DESIGN_COLOR_TOKENS.filter((token) => !new RegExp(`--${token}:\\s*#[0-9a-f]{6,8};`).test(body));
        expect(missing).toEqual([]);
    });

    it("maps every token into Tailwind utilities", () => {
        const theme = block("@theme inline");
        const unmapped = DESIGN_COLOR_TOKENS.filter((token) => !theme.includes(`--color-${token}: var(--${token});`));
        expect(unmapped).toEqual([]);
        for (const [name, px] of [
            ["radius-sm", 4],
            ["radius-md", 6],
            ["radius-lg", 8],
            ["radius-xl", 12],
            ["text-3xs", 10],
            ["text-2xs", 11],
            ["text-4xl", 36],
        ] as const) {
            expect(theme).toContain(`--${name}: ${px}px;`);
        }
    });

    it("uses the design's status colours (current axis) and drops the v1 palette", () => {
        expect(block(".dark")).toContain("--success: #2dd4a7;");
        expect(block(":root")).toContain("--success: #12805f;");
        expect(block(".dark")).toContain("--destructive: #ff4757;");
        expect(block(":root")).toContain("--warning: #b45309;");
        expect(css).not.toMatch(/accent-green|accent-cyan|bg-elevated|\.grain/);
    });
});
