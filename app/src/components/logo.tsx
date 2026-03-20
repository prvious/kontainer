import { Container } from "lucide-react";

export function Logo() {
    return (
        <div className='flex items-center gap-2'>
            <Container className='size-4 text-accent-green' />
            <span className='font-mono text-[13px] font-bold tracking-[1.5px] text-foreground'>KONTAINER</span>
        </div>
    );
}
