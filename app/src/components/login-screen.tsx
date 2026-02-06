import { useState } from "react";
import { Github } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { useAuth } from "@/hooks/use-auth";

export function LoginScreen() {
    const { login } = useAuth();
    const [agentUrl, setAgentUrl] = useState("http://localhost:8080");

    function handleSubmit(e: React.FormEvent) {
        e.preventDefault();
        login(agentUrl.replace(/\/+$/, ""));
    }

    return (
        <div className='flex min-h-screen items-center justify-center p-4'>
            <Card className='w-full max-w-sm'>
                <CardHeader>
                    <CardTitle>Sign in to Devoper</CardTitle>
                    <CardDescription>Connect to your agent to get started.</CardDescription>
                </CardHeader>
                <CardContent>
                    <form onSubmit={handleSubmit} className='flex flex-col gap-4'>
                        <div className='flex flex-col gap-1.5'>
                            <label htmlFor='agent-url' className='text-sm font-medium'>
                                Agent URL
                            </label>
                            <Input
                                id='agent-url'
                                type='url'
                                placeholder='http://localhost:8080'
                                value={agentUrl}
                                onChange={(e) => setAgentUrl(e.target.value)}
                                required
                            />
                        </div>
                        <Button type='submit' className='w-full gap-2'>
                            <Github className='size-4' />
                            Sign in with GitHub
                        </Button>
                    </form>
                </CardContent>
            </Card>
        </div>
    );
}
