export function Logo() {
    return (
        <div className='flex flex-col items-center'>
            <div className='mb-2.5 flex flex-col gap-1'>
                <div className='h-2 w-8 rounded-sm bg-indigo-500' />
                <div className='h-2 w-6.5 rounded-sm bg-indigo-400' />
                <div className='h-2 w-5 rounded-sm bg-indigo-300' />
            </div>
            <span className='text-base font-semibold tracking-tight text-foreground'>devoper</span>
        </div>
    );
}
