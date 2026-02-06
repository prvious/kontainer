import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

// Mock @tauri-apps/api/core
vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn(),
}));

// Mock @tauri-apps/plugin-deep-link
vi.mock("@tauri-apps/plugin-deep-link", () => ({
    onOpenUrl: vi.fn(() => Promise.resolve(() => {})),
}));

// Mock @tauri-apps/plugin-opener
vi.mock("@tauri-apps/plugin-opener", () => ({
    openUrl: vi.fn(),
}));
