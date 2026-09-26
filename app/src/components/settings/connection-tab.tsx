import { useAuth } from "@/hooks/use-auth";

export function ConnectionSection() {
    const { session } = useAuth();

    if (!session) {
        return (
            <section>
                <h2 className='mb-3 border-b border-border pb-2 font-mono text-xs tracking-widest text-muted-foreground uppercase'>Connection</h2>
                <p className='py-3 text-sm text-muted-foreground'>No active connection. Connect to a controller first.</p>
            </section>
        );
    }

    return (
        <section>
            <h2 className='mb-3 border-b border-border pb-2 font-mono text-xs tracking-widest text-muted-foreground uppercase'>Connection</h2>
            <div className='divide-y divide-border/50'>
                <div className='flex items-center justify-between py-3'>
                    <span className='text-sm text-muted-foreground'>Display name</span>
                    <span className='text-sm text-foreground'>{session.label ?? session.githubLogin}</span>
                </div>
                <div className='flex items-center justify-between gap-4 py-3'>
                    <span className='text-sm text-muted-foreground'>Controller URL</span>
                    <span className='truncate font-mono text-sm text-foreground'>{session.origin}</span>
                </div>
            </div>
        </section>
    );
}
