import { useState } from "react";
import { ChevronDown } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { useAuth } from "@/hooks/use-auth";

export function ClusterSwitcher({ variant = "sidebar" }: { variant?: "sidebar" | "titlebar" }) {
    const { session, logout } = useAuth();
    const [open, setOpen] = useState(false);
    const label = session?.label ?? session?.githubLogin ?? "No connection";

    async function handleLogout() {
        try {
            await logout();
            setOpen(false);
        } catch {
            // The provider exposes a closed error code and clears the session
            // only when Rust has left authenticated state.
            setOpen(false);
        }
    }

    const trigger =
        variant === "titlebar" ? (
            <button className='flex items-center gap-2 text-text-secondary transition-colors hover:text-foreground'>
                <span className='size-[7px] shrink-0 rounded-full bg-accent-green' />
                <span className='font-mono text-[11px] font-medium tracking-[0.5px]'>{label}</span>
                <ChevronDown className='size-3 text-muted-foreground' />
            </button>
        ) : (
            <button className='flex w-full items-center gap-3 rounded-md p-1.5 text-left transition-colors hover:bg-accent'>
                <span className='size-2 shrink-0 rounded-full bg-accent-green' />
                <div className='flex-1 overflow-hidden'>
                    <p className='truncate text-sm font-semibold'>{label}</p>
                    <span className='truncate font-mono text-[11px] text-muted-foreground'>
                        {session?.origin.replace(/^https?:\/\//, "") ?? "Connect a controller"}
                    </span>
                </div>
                <ChevronDown className={`size-4 shrink-0 text-muted-foreground transition-transform ${open ? "rotate-180" : ""}`} />
            </button>
        );

    return (
        <Popover open={open} onOpenChange={setOpen}>
            <PopoverTrigger render={trigger} />
            <PopoverContent side={variant === "titlebar" ? "bottom" : "top"} align='center' className='w-[300px] space-y-3 border-border bg-card p-3'>
                <div>
                    <p className='truncate text-sm font-semibold'>{label}</p>
                    <p className='truncate font-mono text-[11px] text-muted-foreground'>{session?.origin}</p>
                </div>
                <Button
                    variant='outline'
                    size='sm'
                    className='w-full'
                    onClick={() => {
                        void handleLogout();
                    }}
                >
                    Sign out
                </Button>
            </PopoverContent>
        </Popover>
    );
}
