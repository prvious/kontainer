import { Link, createFileRoute } from "@tanstack/react-router";

function HomePage() {
    return (
        <div className='mx-auto max-w-md space-y-4 p-6'>
            <h1 className='text-xl font-semibold'>Connected</h1>
            <p className='text-sm text-muted-foreground'>Browse the configured node’s containers.</p>
            <Link
                to='/tasks'
                className='inline-flex h-8 items-center rounded-lg bg-primary px-3 text-sm font-medium text-primary-foreground hover:bg-primary/80'
            >
                Open Containers
            </Link>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/")({ component: HomePage });
