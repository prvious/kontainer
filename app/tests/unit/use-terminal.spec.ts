import { describe, it, expect } from "vitest";
import { renderHook } from "@testing-library/react";
import { createElement } from "react";
import { useTerminal } from "@/hooks/use-terminal";
import { TerminalProvider } from "@/contexts/terminal-context";

describe("useTerminal", () => {
    it("throws when used outside TerminalProvider", () => {
        expect(() => {
            renderHook(() => useTerminal());
        }).toThrow("useTerminal must be used within a TerminalProvider");
    });

    it("provides openExec and closeExec", () => {
        const wrapper = ({ children }: { children: React.ReactNode }) => createElement(TerminalProvider, null, children);

        const { result } = renderHook(() => useTerminal(), { wrapper });

        expect(result.current.openExec).toBeInstanceOf(Function);
        expect(result.current.closeExec).toBeInstanceOf(Function);
        expect(result.current.target).toBeNull();
    });
});
