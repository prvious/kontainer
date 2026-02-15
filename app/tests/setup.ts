import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

// Mock @tauri-apps/api/core
vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn(),
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

// Mock useSettings so all hooks that depend on SettingsProvider work in tests
vi.mock("@/hooks/use-settings", () => ({
    useSettings: () => ({
        refreshInterval: 5000,
        setRefreshInterval: vi.fn(),
        terminalDefaults: {
            shell: "",
            fontSize: 13,
            fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', Menlo, Monaco, 'Courier New', monospace",
            scrollback: 1000,
        },
        setTerminalDefaults: vi.fn(),
        isLoading: false,
    }),
}));

// Mock useAppearance so components using AppearanceProvider work in tests
vi.mock("@/hooks/use-appearance", () => ({
    useAppearance: () => ({
        scheme: "system",
        setScheme: vi.fn(),
        resolvedScheme: "light",
        theme: "default",
        setTheme: vi.fn(),
        themes: [{ id: "default", label: "Default" }],
        font: "figtree",
        setFont: vi.fn(),
        fonts: [
            { id: "figtree", label: "Figtree", cssFamily: '"Figtree Variable", sans-serif', load: vi.fn() },
            { id: "inter", label: "Inter", cssFamily: '"Inter Variable", sans-serif', load: vi.fn() },
        ],
        isFontLoading: false,
    }),
}));
