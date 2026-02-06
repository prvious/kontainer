import { createFileRoute } from "@tanstack/react-router";
import { useAuth } from "@/hooks/use-auth";
import { Button } from "@/components/ui/button";
import { LogOut } from "lucide-react";

function DashboardPage() {
    const { user, logout } = useAuth();

    return (
        <div className='flex min-h-screen flex-col'>
            <header className='flex items-center justify-between border-b px-4 py-2'>
                <div className='flex items-center gap-3'>
                    {user?.avatarUrl && <img src={user.avatarUrl} alt={user.name} className='size-7 rounded-full' />}
                    <span className='text-sm font-medium'>{user?.name}</span>
                </div>
                <Button variant='ghost' size='sm' onClick={logout}>
                    <LogOut className='size-4' />
                    Sign out
                </Button>
            </header>
            <main className='flex flex-1 items-center justify-center p-4'>
                <p className='text-sm text-muted-foreground'>Connected as {user?.email}</p>
            </main>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/")({
    component: DashboardPage,
});
