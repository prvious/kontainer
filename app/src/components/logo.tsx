import { cn } from "@/lib/utils";

/**
 * Logo Mark `g54PvW` (9 bars) and Logo Mark Small `WgGUD` (5 bars): stacked bars alternating
 * `--primary-deep` and `--primary`.
 */
export function LogoMark({ size = "sm", className }: { size?: "sm" | "lg"; className?: string }) {
    const bars = size === "sm" ? 5 : 9;
    return (
        <span aria-hidden className={cn("flex shrink-0 flex-col justify-center", size === "sm" ? "size-4 gap-px px-px" : "size-11 gap-px px-0.75", className)}>
            {Array.from({ length: bars }, (_, index) => (
                <span key={index} className={cn("w-full", size === "sm" ? "h-0.5" : "h-1", index % 2 === 0 ? "bg-primary-deep" : "bg-primary")} />
            ))}
        </span>
    );
}

/** Logo Mark Small plus the KONTAINER word mark in the brand font. */
export function Logo() {
    return (
        <span className='flex items-center gap-2'>
            <LogoMark />
            <span className='font-brand text-sm font-black tracking-[1px] text-foreground'>KONTAINER</span>
        </span>
    );
}
