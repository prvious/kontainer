// import { useMatches } from "@tanstack/react-router";
import { Search } from "lucide-react";
import { useCommandPalette } from "@/hooks/use-command-palette";

// const routeLabels: Record<string, string> = {
//     "/": "Overview",
//     "/services": "Services",
//     "/tasks": "Containers",
//     "/nodes": "Nodes",
//     "/stacks": "Stacks",
//     "/networks": "Networks",
//     "/volumes": "Volumes",
//     "/logs": "Logs",
//     "/settings": "Settings",
//     "/secrets": "Secrets",
//     "/configs": "Configs",
//     "/images": "Images",
// };

export function DashboardHeader() {
    const { setOpen } = useCommandPalette();
    // const matches = useMatches();

    // const currentPath = matches[matches.length - 1]?.fullPath ?? "/";
    // const basePath = Object.keys(routeLabels).find((k) => currentPath === k || (k !== "/" && currentPath.startsWith(k))) ?? "/";
    // const currentLabel = routeLabels[basePath] ?? "Overview";

    return (
        <header data-tauri-drag-region className='flex shrink-0 items-center gap-4 pb-2'>
            {/* <h1 className='text-sm font-semibold'>{currentLabel}</h1> */}

            <div className='flex-1' />

            <button
                onClick={() => setOpen(true)}
                className='flex items-center justify-center gap-2 rounded-full border border-border bg-muted/50 px-3 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground'
            >
                <Search className='size-3' />
                <span>Commands...</span>
                <kbd className='ml-2 font-mono text-[10px] text-muted-foreground/60'>⌘K</kbd>
            </button>
        </header>
    );
}
