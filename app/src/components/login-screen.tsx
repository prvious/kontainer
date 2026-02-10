import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getRouteApi, useNavigate } from "@tanstack/react-router";
import { AlertTriangle, ArrowRight, CheckCircle2, Globe, Lock, Loader2, Shield, WifiOff } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Logo } from "@/components/logo";
import { useAuth } from "@/hooks/use-auth";

const routeApi = getRouteApi("/login");

type ConnectionStatus = "idle" | "connecting" | "online" | "offline" | "error";

const statusConfig = {
    idle: {
        icon: Globe,
        color: "text-muted-foreground",
        bg: "bg-muted",
    },
    connecting: {
        icon: Loader2,
        color: "text-chart-2",
        bg: "bg-chart-2/10",
    },
    online: {
        icon: CheckCircle2,
        color: "text-success",
        bg: "bg-success/10",
    },
    offline: {
        icon: WifiOff,
        color: "text-destructive",
        bg: "bg-destructive/10",
    },
    error: {
        icon: AlertTriangle,
        color: "text-warning",
        bg: "bg-warning/10",
    },
} as const;

function GitHubIcon({ className }: { className?: string }) {
    return (
        <svg viewBox='0 0 24 24' className={className} fill='currentColor'>
            <path d='M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z' />
        </svg>
    );
}

export function LoginScreen() {
    const { login } = useAuth();
    const navigate = useNavigate();
    const { redirect: redirectTo } = routeApi.useSearch();
    const [agentUrl, setAgentUrl] = useState("http://localhost:8080");
    const [status, setStatus] = useState<ConnectionStatus>("idle");
    const [errorMessage, setErrorMessage] = useState<string | null>(null);
    const [providers, setProviders] = useState<string[]>([]);
    const [authenticatingProvider, setAuthenticatingProvider] = useState<string | null>(null);

    function handleUrlChange(e: React.ChangeEvent<HTMLInputElement>) {
        setAgentUrl(e.target.value);
        if (status !== "idle") {
            setStatus("idle");
            setErrorMessage(null);
            setProviders([]);
            setAuthenticatingProvider(null);
        }
    }

    async function handleConnect(e: React.FormEvent) {
        e.preventDefault();
        const url = agentUrl.replace(/\/+$/, "");

        setStatus("connecting");
        setErrorMessage(null);
        setProviders([]);
        setAuthenticatingProvider(null);

        try {
            await invoke("ping_agent", { url });
        } catch (err) {
            setStatus("offline");
            setErrorMessage(typeof err === "string" ? err : "Could not reach the cluster");
            return;
        }

        setStatus("online");

        try {
            const result = await invoke<string[]>("fetch_providers", { url });
            setProviders(result);
        } catch (err) {
            setStatus("error");
            setErrorMessage(typeof err === "string" ? err : "Failed to fetch auth providers");
        }
    }

    async function handleProviderClick(provider: string) {
        const url = agentUrl.replace(/\/+$/, "");
        setAuthenticatingProvider(provider);
        try {
            await login(url, undefined, provider);
            navigate({ to: redirectTo || "/" });
        } catch (err) {
            setErrorMessage(err instanceof Error ? err.message : "Authentication failed");
            setAuthenticatingProvider(null);
        }
    }

    const currentStatus = statusConfig[status];
    const StatusIcon = currentStatus.icon;

    return (
        <div className='relative flex min-h-screen flex-col items-center justify-center px-4'>
            {/* Grid background */}
            <div className='absolute inset-0 bg-[linear-gradient(rgba(255,255,255,0.03)_1px,transparent_1px),linear-gradient(90deg,rgba(255,255,255,0.03)_1px,transparent_1px)] bg-size-[64px_64px]' />

            <div className='relative z-10 flex w-full max-w-md flex-col items-center'>
                {/* Logo */}
                <div className='mb-10'>
                    <Logo />
                </div>

                {/* Card */}
                <Card className='w-full'>
                    <CardHeader>
                        <CardTitle>Connect to Cluster</CardTitle>
                        <CardDescription>Enter your agent's URL to get started.</CardDescription>
                    </CardHeader>
                    <CardContent>
                        {/* URL Input */}
                        <form onSubmit={handleConnect}>
                            <Label htmlFor='cluster-url' className='mb-2'>
                                <Globe className='h-4 w-4 text-muted-foreground' />
                                Cluster URL
                            </Label>
                            <div className='relative'>
                                <Input
                                    id='cluster-url'
                                    type='url'
                                    placeholder='http://localhost:8080'
                                    value={agentUrl}
                                    onChange={handleUrlChange}
                                    required
                                    className='h-11 pr-10 font-mono'
                                />
                                <div className='absolute top-1/2 right-3 -translate-y-1/2'>
                                    <StatusIcon className={`h-4 w-4 ${currentStatus.color} ${status === "connecting" ? "animate-spin" : ""}`} />
                                </div>
                            </div>
                            <p className='mt-2 text-xs text-muted-foreground'>The public endpoint of your Swarm manager node</p>

                            {/* Connect / Retry Button */}
                            {status !== "online" && (
                                <Button type='submit' disabled={status === "connecting"} size='default' className='mt-4 w-full'>
                                    {status === "connecting" ? (
                                        <>
                                            <Loader2 className='h-4 w-4 animate-spin' />
                                            Reaching cluster...
                                        </>
                                    ) : status === "offline" || status === "error" ? (
                                        "Retry Connection"
                                    ) : (
                                        "Connect"
                                    )}
                                </Button>
                            )}
                        </form>

                        {/* Status Banner */}
                        {(status === "online" || status === "offline" || status === "error") && (
                            <div className={`mt-4 flex items-center gap-3 rounded-lg p-3 ${currentStatus.bg}`}>
                                <StatusIcon className={`h-5 w-5 shrink-0 ${currentStatus.color}`} />
                                <div className='min-w-0 flex-1'>
                                    {status === "online" ? (
                                        <p className={`text-sm font-medium ${currentStatus.color}`}>Cluster Online</p>
                                    ) : (
                                        <p className={`font-mono text-xs break-all ${currentStatus.color}`}>{errorMessage}</p>
                                    )}
                                </div>
                            </div>
                        )}

                        {/* Auth Provider Buttons */}
                        {status === "online" && providers.length > 0 && (
                            <div className='mt-6'>
                                <div className='relative flex items-center gap-4'>
                                    <div className='h-px flex-1 bg-border' />
                                    <span className='text-xs text-muted-foreground'>authenticate to continue</span>
                                    <div className='h-px flex-1 bg-border' />
                                </div>

                                <div className='mt-4 flex flex-col gap-2'>
                                    {providers.map((provider) => {
                                        const isAuthenticating = authenticatingProvider === provider;
                                        const isDisabled = authenticatingProvider !== null;
                                        const ProviderIcon = provider === "github" ? GitHubIcon : Shield;

                                        return (
                                            <Button
                                                key={provider}
                                                type='button'
                                                variant='outline'
                                                size='lg'
                                                disabled={isDisabled}
                                                onClick={() => handleProviderClick(provider)}
                                                className='w-full'
                                            >
                                                {isAuthenticating ? (
                                                    <>
                                                        <Loader2 className='h-4 w-4 animate-spin' />
                                                        Authenticating...
                                                    </>
                                                ) : (
                                                    <>
                                                        <ProviderIcon className='h-4 w-4' />
                                                        Continue with {provider.charAt(0).toUpperCase() + provider.slice(1)}
                                                        <ArrowRight className='ml-auto h-4 w-4 text-muted-foreground' />
                                                    </>
                                                )}
                                            </Button>
                                        );
                                    })}
                                </div>
                            </div>
                        )}
                    </CardContent>
                </Card>

                {/* TLS hint */}
                <div className='mt-3 flex items-center justify-center gap-1.5 text-xs text-muted-foreground'>
                    <Lock className='h-3 w-3' />
                    <span>TLS encrypted</span>
                </div>

                {/* Help Text */}
                <p className='mt-3 text-center text-xs text-muted-foreground'>
                    Enter the URL of your devoper agent.{" "}
                    <a href='https://prvious.dev' target='_blank' rel='noopener noreferrer' className='underline underline-offset-2 hover:text-foreground'>
                        Need help? Check the setup guide.
                    </a>
                </p>
            </div>

            {/* Footer */}
            <p className='absolute bottom-6 text-xs text-muted-foreground'>
                End-to-end encrypted &middot; Built by{" "}
                <a href='https://prvious.dev' target='_blank' rel='noopener noreferrer' className='underline underline-offset-2 hover:text-foreground'>
                    prvious.dev
                </a>
            </p>
        </div>
    );
}
