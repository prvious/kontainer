import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

export function useNativeBehavior() {
    useEffect(() => {
        const isDev = import.meta.env.DEV;

        function handleContextMenu(e: MouseEvent) {
            if (isDev) return;
            e.preventDefault();
        }

        function handleKeyDown(e: KeyboardEvent) {
            const mod = e.metaKey || e.ctrlKey;

            // Block refresh: Cmd+R, Ctrl+R, F5, Cmd+Shift+R, Ctrl+Shift+R
            if ((mod && e.key === "r") || e.key === "F5") {
                e.preventDefault();
                return;
            }

            // Block print: Cmd+P, Ctrl+P
            if (mod && e.key === "p") {
                e.preventDefault();
                return;
            }

            // Block zoom: Cmd+Plus, Cmd+Minus, Cmd+0
            if (mod && (e.key === "=" || e.key === "+" || e.key === "-" || e.key === "0")) {
                e.preventDefault();
                return;
            }
        }

        function handleWheel(e: WheelEvent) {
            // Block pinch-to-zoom / Ctrl+scroll zoom
            if (e.ctrlKey || e.metaKey) {
                e.preventDefault();
            }
        }

        function handleDragOver(e: DragEvent) {
            e.preventDefault();
        }

        function handleDrop(e: DragEvent) {
            e.preventDefault();
        }

        function handleMouseDown(e: MouseEvent) {
            // Only primary (left) button
            if (e.buttons !== 1) return;
            const target = e.target as HTMLElement;
            // Skip interactive elements
            if (target.closest("button, a, input, select, textarea, [role='button'], [role='menuitem']")) return;
            // Check if within a drag region
            if (target.closest("[data-tauri-drag-region]")) {
                e.preventDefault();
                getCurrentWindow().startDragging();
            }
        }

        document.addEventListener("contextmenu", handleContextMenu);
        document.addEventListener("keydown", handleKeyDown);
        document.addEventListener("wheel", handleWheel, { passive: false });
        document.addEventListener("dragover", handleDragOver);
        document.addEventListener("drop", handleDrop);
        document.addEventListener("mousedown", handleMouseDown);

        return () => {
            document.removeEventListener("contextmenu", handleContextMenu);
            document.removeEventListener("keydown", handleKeyDown);
            document.removeEventListener("wheel", handleWheel);
            document.removeEventListener("dragover", handleDragOver);
            document.removeEventListener("drop", handleDrop);
            document.removeEventListener("mousedown", handleMouseDown);
        };
    }, []);
}
