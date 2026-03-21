interface PageHeaderProps {
    title: string;
    subtitle: string;
    children?: React.ReactNode;
}

export function PageHeader({ title, subtitle, children }: PageHeaderProps) {
    return (
        <div className='flex items-center justify-between'>
            <div className='flex flex-col gap-1'>
                <h1 className='text-2xl font-bold tracking-tight text-foreground'>{title}</h1>
                <p className='font-mono text-[11px] tracking-[0.3px] text-muted-foreground'>{subtitle}</p>
            </div>
            {children && <div className='flex items-center gap-2'>{children}</div>}
        </div>
    );
}
