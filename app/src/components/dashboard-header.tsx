import { Search, Bell } from "lucide-react";
import { Logo } from "@/components/logo";
import { useCommandPalette } from "@/hooks/use-command-palette";
import { ClusterSwitcher } from "@/components/cluster-switcher";

export function DashboardHeader() {
    const { setOpen } = useCommandPalette();

    return (
        <header data-tauri-drag-region className='flex h-[38px] shrink-0 items-center justify-between border-b border-border bg-titlebar-bg/90 px-4'>
            {/* Left: native traffic lights spacer + logo */}
            <div className='flex items-center gap-3'>
                <div className='w-[52px] shrink-0' />
                <Logo />
            </div>

            {/* Center: cluster switcher */}
            <ClusterSwitcher variant='titlebar' />

            {/* Right: utility icons */}
            <div className='flex items-center gap-4'>
                <button className='text-muted-foreground transition-colors hover:text-foreground' onClick={() => setOpen(true)}>
                    <Search className='size-3.5' />
                </button>
                <button className='text-muted-foreground transition-colors hover:text-foreground'>
                    <Bell className='size-3.5' />
                </button>
            </div>
        </header>
    );
}
