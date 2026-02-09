import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

// Mock @tauri-apps/api/core
vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn(),
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
        isLoading: false,
    }),
}));
