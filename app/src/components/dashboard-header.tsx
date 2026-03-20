import { Search, Bell, ChevronDown } from "lucide-react";
import { Logo } from "@/components/logo";
import { useAuth } from "@/hooks/use-auth";
import { useCommandPalette } from "@/hooks/use-command-palette";

export function DashboardHeader() {
    const { profiles, activeProfileId } = useAuth();
    const { setOpen } = useCommandPalette();
    const activeProfile = profiles.find((p) => p.id === activeProfileId);
    const clusterName = activeProfile?.label ?? "No cluster";

    return (
        <header
            data-tauri-drag-region
            className='flex h-[38px] shrink-0 items-center justify-between border-b border-border bg-titlebar-bg/90 px-4'
        >
            {/* Left: traffic lights spacer + logo */}
            <div className='flex items-center gap-3'>
                <div className='flex items-center gap-2'>
                    <span className='size-3 rounded-full bg-[#FF5F57]' />
                    <span className='size-3 rounded-full bg-[#FEBC2E]' />
                    <span className='size-3 rounded-full bg-[#28C840]' />
                </div>
                <Logo />
            </div>

            {/* Center: cluster selector */}
            <button
                className='flex items-center gap-2 text-text-secondary transition-colors hover:text-foreground'
                onClick={() => setOpen(true)}
            >
                <span className='size-[7px] shrink-0 rounded-full bg-accent-green animate-pulse-dot' />
                <span className='font-mono text-[11px] font-medium tracking-[0.5px]'>
                    {clusterName}
                </span>
                <ChevronDown className='size-3 text-muted-foreground' />
            </button>

            {/* Right: utility icons */}
            <div className='flex items-center gap-4'>
                <button
                    className='text-muted-foreground transition-colors hover:text-foreground'
                    onClick={() => setOpen(true)}
                >
                    <Search className='size-3.5' />
                </button>
                <button className='text-muted-foreground transition-colors hover:text-foreground'>
                    <Bell className='size-3.5' />
                </button>
            </div>
        </header>
    );
}
