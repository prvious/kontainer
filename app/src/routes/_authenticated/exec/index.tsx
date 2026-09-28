import { createFileRoute } from "@tanstack/react-router";

function ExecPage() {
    return <p className='p-6 text-sm text-muted-foreground'>Exec will be available after its native client flow is ported.</p>;
}

export const Route = createFileRoute("/_authenticated/exec/")({ component: ExecPage });
