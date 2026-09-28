import { connectionHost } from "@/components/connect/connect-rules";
import { useAuth } from "@/hooks/use-auth";

function Row({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
    return (
        <div className='flex items-center justify-between gap-4 py-3'>
            <span className='text-sm text-muted-foreground'>{label}</span>
            <span className={mono ? 'truncate font-mono text-sm text-foreground' : 'truncate text-sm text-foreground'}>{value}</span>
        </div>
    );
}

/** Settings › Connection: the signed-in agent and identity, read-only. */
export function ConnectionSection() {
    const { session } = useAuth();

    return (
        <section>
            <h2 className='mb-3 border-b border-border pb-2 font-mono text-xs tracking-widest text-muted-foreground uppercase'>Connection</h2>
            {session ? (
                <div className='divide-y divide-border/50'>
                    <Row label='Label' value={session.label ?? connectionHost(session.origin)} />
                    <Row label='Agent URL' value={session.origin} mono />
                    <Row label='Email' value={session.email} mono />
                    <Row label='Display name' value={session.displayName} />
                </div>
            ) : (
                <p className='py-3 text-sm text-muted-foreground'>No active connection. Connect to a controller first.</p>
            )}
        </section>
    );
}
