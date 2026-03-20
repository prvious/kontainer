import { Container } from "lucide-react";

export function SplashScreen() {
    return (
        <div className='flex h-screen flex-col bg-background'>
            {/* Accent line */}
            <div
                className='h-0.5 w-full shrink-0'
                style={{ background: "linear-gradient(90deg, #00FF88, #00D4FF 50%, #A78BFA)" }}
            />

            <div className='flex flex-1 items-center justify-center'>
                <div className='flex w-[400px] flex-col items-center gap-7'>
                    {/* Logo */}
                    <div className='flex size-[88px] items-center justify-center rounded-[20px] border-[1.5px] border-accent-green/40 bg-gradient-to-b from-accent-green/15 to-accent-cyan/8'>
                        <Container className='size-11 text-accent-green' />
                    </div>

                    {/* Brand */}
                    <div className='flex flex-col items-center gap-2.5'>
                        <span className='text-[38px] font-bold text-foreground'>kontainer</span>
                        <span className='font-mono text-sm font-medium text-text-secondary'>Infrastructure, Simplified</span>
                    </div>

                    {/* Loading */}
                    <div className='flex w-[280px] flex-col items-center gap-3'>
                        <div className='h-[3px] w-full overflow-hidden rounded-sm bg-bg-subtle'>
                            <div className='h-full w-1/3 animate-pulse rounded-sm bg-gradient-to-r from-accent-green to-accent-cyan' />
                        </div>
                        <span className='font-mono text-[11px] font-medium text-muted-foreground'>Initializing runtime...</span>
                    </div>

                    {/* Version */}
                    <span className='font-mono text-[10px] font-medium text-muted-foreground'>v1.0.0</span>
                </div>
            </div>
        </div>
    );
}
