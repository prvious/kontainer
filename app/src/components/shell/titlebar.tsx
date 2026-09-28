import type { ReactNode } from "react";
import { Logo } from "@/components/logo";

/**
 * Titlebar `iHwhS` over the transparent native title bar. The macOS traffic lights sit in the
 * reserved space on the left; the search and bell icons are not rendered (design-gaps G13).
 */
export function Titlebar({ center }: { center?: ReactNode }) {
    return (
        <header data-tauri-drag-region className='relative flex h-9.5 shrink-0 items-center border-b border-border bg-titlebar px-4'>
            <div className='flex items-center gap-3'>
                <span aria-hidden className='w-13 shrink-0' />
                <Logo />
            </div>
            {center && <div className='absolute left-1/2 -translate-x-1/2'>{center}</div>}
        </header>
    );
}
