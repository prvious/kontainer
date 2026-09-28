import { readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

const SOURCE_ROOT = path.resolve(__dirname, "../../src");
const FORBIDDEN_IMPORT_FRAGMENTS = ["/actions/agent/", "use-log-stream", "command-palette", "setup-wizard"];

function sourceFiles(directory: string): string[] {
    return readdirSync(directory).flatMap((entry) => {
        const fullPath = path.join(directory, entry);
        if (statSync(fullPath).isDirectory()) return sourceFiles(fullPath);
        return /\.(ts|tsx)$/.test(entry) ? [fullPath] : [];
    });
}

function importedPaths(source: string): string[] {
    const pattern = /(?:import|export)\s[^'"]*?from\s*["']([^"']+)["']|import\(\s*["']([^"']+)["']\s*\)/g;
    return [...source.matchAll(pattern)].map((match) => match[1] ?? match[2]);
}

describe("deleted v1 client code", () => {
    it("is not imported anywhere under app/src", () => {
        const offenders = sourceFiles(SOURCE_ROOT).flatMap((file) =>
            importedPaths(readFileSync(file, "utf8"))
                .filter((imported) => FORBIDDEN_IMPORT_FRAGMENTS.some((fragment) => imported.includes(fragment)))
                .map((imported) => `${path.relative(SOURCE_ROOT, file)} imports ${imported}`),
        );
        expect(offenders).toEqual([]);
    });

    it("recognises the import forms it guards against", () => {
        expect(importedPaths(`import { a } from "@/hooks/use-log-stream";\nconst b = import("./command-palette");`)).toEqual([
            "@/hooks/use-log-stream",
            "./command-palette",
        ]);
    });
});
