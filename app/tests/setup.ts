import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

// Mock @tauri-apps/api/core
vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn(),
    // Tests take the channel from the invoke arguments and call `onmessage` to push events.
    Channel: class {
        onmessage: (message: unknown) => void = () => {};
        constructor(onmessage?: (message: unknown) => void) {
            if (onmessage) this.onmessage = onmessage;
        }
    },
}));

// Mock @tauri-apps/api/app
vi.mock("@tauri-apps/api/app", () => ({
    getVersion: vi.fn(() => Promise.resolve("0.1.0")),
}));

// Mock @tauri-apps/api/window
vi.mock("@tauri-apps/api/window", () => ({
    getCurrentWindow: () => ({
        theme: vi.fn(() => Promise.resolve("light")),
        setTheme: vi.fn(() => Promise.resolve()),
        onThemeChanged: vi.fn(() => Promise.resolve(() => {})),
    }),
}));

// Mock @tauri-apps/api/event
vi.mock("@tauri-apps/api/event", () => ({
    listen: vi.fn(() => Promise.resolve(() => {})),
}));

// Mock @tauri-apps/plugin-opener
vi.mock("@tauri-apps/plugin-opener", () => ({
    openUrl: vi.fn(),
}));

// Mock useSettings so hooks that read polling and terminal defaults work in tests
vi.mock("@/hooks/use-settings", () => ({
    useSettings: vi.fn(() => ({
        refreshInterval: 5000,
        terminalDefaults: {
            shell: "",
            fontSize: 13,
            fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', Menlo, Monaco, 'Courier New', monospace",
            scrollback: 1000,
        },
        isLoading: false,
    })),
}));

// Mock useAppearance so components outside AppearanceProvider work in tests
vi.mock("@/hooks/use-appearance", () => ({
    useAppearance: vi.fn(() => ({
        scheme: "system",
        setScheme: vi.fn(),
        resolvedScheme: "dark",
    })),
}));

// jsdom has no matchMedia or ResizeObserver; the shadcn sidebar and Recharts read them.
Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: (query: string) => ({
        matches: false,
        media: query,
        onchange: null,
        addEventListener: () => {},
        removeEventListener: () => {},
        addListener: () => {},
        removeListener: () => {},
        dispatchEvent: () => false,
    }),
});

class ResizeObserverStub {
    observe() {}
    unobserve() {}
    disconnect() {}
}
globalThis.ResizeObserver ??= ResizeObserverStub as unknown as typeof ResizeObserver;
