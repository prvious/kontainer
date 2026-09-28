import { useState } from "react";
import { ChevronDown } from "lucide-react";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { ActionButton } from "@/components/kit/action-button";
import { StatusDot } from "@/components/kit/badges";
import { useAuth } from "@/hooks/use-auth";

/** The titlebar centre pill (status dot, connection label, chevron) and its switcher popover. */
export function ClusterSwitcher({ variant = "titlebar" }: { variant?: "titlebar" }) {
    const { session, logout } = useAuth();
    const [open, setOpen] = useState(false);
    const label = session?.label ?? session?.githubLogin ?? "No connection";

    async function handleLogout() {
        try {
            await logout();
        } catch {
            // The provider exposes a closed error code and clears the session
            // only when Rust has left authenticated state.
        } finally {
            setOpen(false);
        }
    }

    return (
        <Popover open={open} onOpenChange={setOpen}>
            <PopoverTrigger
                data-variant={variant}
                className='flex items-center gap-2 rounded-sm px-2 py-1 font-mono text-2xs font-medium tracking-[0.5px] text-secondary-foreground transition-colors hover:text-foreground'
            >
                <StatusDot tone='success' />
                <span>{label}</span>
                <ChevronDown aria-hidden className='size-3 text-muted-foreground' />
            </PopoverTrigger>
            <PopoverContent side='bottom' align='center' className='w-75 gap-3 border-border bg-popover p-3'>
                <div className='min-w-0'>
                    <p className='truncate font-mono text-xs font-semibold text-foreground'>{label}</p>
                    <p className='truncate font-mono text-2xs text-muted-foreground'>{session?.origin}</p>
                </div>
                <ActionButton
                    variant='outline'
                    className='w-full'
                    onClick={() => {
                        void handleLogout();
                    }}
                >
                    Sign out
                </ActionButton>
            </PopoverContent>
        </Popover>
    );
}
