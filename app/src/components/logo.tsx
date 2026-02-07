export function Logo() {
    return (
        <div className='flex items-center gap-2.5'>
            <div className='flex flex-col gap-0.5'>
                <div className='h-1.5 w-5 rounded-sm bg-foreground' />
                <div className='h-1.5 w-4 rounded-sm bg-foreground/70' />
                <div className='h-1.5 w-3 rounded-sm bg-foreground/40' />
            </div>
            <span className='text-sm font-semibold tracking-tight'>devoper</span>
        </div>
    );
}
