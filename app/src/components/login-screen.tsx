import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getRouteApi, useNavigate } from "@tanstack/react-router";
import { AlertTriangle, CheckCircle2, Globe, Loader2, Shield, WifiOff } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useAuth } from "@/hooks/use-auth";
import { SetupWizard } from "@/components/setup-wizard";

const routeApi = getRouteApi("/login");

type ConnectionStatus = "idle" | "connecting" | "online" | "offline" | "error";

const statusConfig = {
    idle: {
        icon: Globe,
        color: "text-muted-foreground",
    },
    connecting: {
        icon: Loader2,
        color: "text-primary",
    },
    online: {
        icon: CheckCircle2,
        color: "text-success",
    },
    offline: {
        icon: WifiOff,
        color: "text-destructive",
    },
    error: {
        icon: AlertTriangle,
        color: "text-warning",
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
    const [agentUrl, setAgentUrl] = useState("http://localhost:3000");
    const [status, setStatus] = useState<ConnectionStatus>("idle");
    const [errorMessage, setErrorMessage] = useState<string | null>(null);
    const [providers, setProviders] = useState<string[]>([]);
    const [authenticatingProvider, setAuthenticatingProvider] = useState<string | null>(null);
    const [setupRequired, setSetupRequired] = useState(false);

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
            const pingResult = await invoke<{ app: string; status: string }>("ping_agent", { url });
            if (pingResult.status === "setup_required") {
                setSetupRequired(true);
                return;
            }
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

    if (setupRequired) {
        return (
            <SetupWizard
                agentUrl={agentUrl.replace(/\/+$/, "")}
                onComplete={() => {
                    setSetupRequired(false);
                    setStatus("idle");
                }}
                onBack={() => {
                    setSetupRequired(false);
                    setStatus("idle");
                }}
            />
        );
    }

    return (
        <div className='flex h-screen p-2 pt-10'>
            <div data-tauri-drag-region className='grain flex flex-1 flex-col items-center justify-center overflow-hidden rounded-xl bg-background shadow-sm'>
                <div className='w-full max-w-sm px-6'>
                    <h1 className='text-lg font-semibold'>Connect to Cluster</h1>
                    <p className='mt-1 text-sm text-muted-foreground'>Enter your agent URL to get started.</p>

                    <form onSubmit={handleConnect} className='mt-6'>
                        <Label htmlFor='cluster-url' className='mb-2'>
                            Agent URL
                        </Label>
                        <div className='relative'>
                            <Input
                                id='cluster-url'
                                type='url'
                                placeholder='http://localhost:3000'
                                value={agentUrl}
                                onChange={handleUrlChange}
                                required
                                className='h-9 pr-9 font-mono text-sm'
                            />
                            <div className='absolute top-1/2 right-3 -translate-y-1/2'>
                                <StatusIcon className={`size-3.5 ${currentStatus.color} ${status === "connecting" ? "animate-spin" : ""}`} />
                            </div>
                        </div>

                        {/* Error message */}
                        {(status === "offline" || status === "error") && errorMessage && (
                            <p className={`mt-2 text-xs ${currentStatus.color}`}>{errorMessage}</p>
                        )}

                        {/* Connect / Retry Button */}
                        {status !== "online" && (
                            <Button type='submit' disabled={status === "connecting"} size='sm' className='mt-3 w-full'>
                                {status === "connecting" ? (
                                    <>
                                        <Loader2 className='size-3.5 animate-spin' />
                                        Connecting...
                                    </>
                                ) : status === "offline" || status === "error" ? (
                                    "Retry"
                                ) : (
                                    "Connect"
                                )}
                            </Button>
                        )}
                    </form>

                    {/* Auth Providers */}
                    {status === "online" && providers.length > 0 && (
                        <div className='mt-6'>
                            <div className='flex items-center gap-3'>
                                <div className='h-px flex-1 bg-border' />
                                <span className='text-xs text-muted-foreground'>Authenticate</span>
                                <div className='h-px flex-1 bg-border' />
                            </div>

                            <div className='mt-3 flex flex-col gap-2'>
                                {providers.map((provider) => {
                                    const isAuthenticating = authenticatingProvider === provider;
                                    const isDisabled = authenticatingProvider !== null;
                                    const ProviderIcon = provider === "github" ? GitHubIcon : Shield;

                                    return (
                                        <Button
                                            key={provider}
                                            type='button'
                                            variant='outline'
                                            size='sm'
                                            disabled={isDisabled}
                                            onClick={() => handleProviderClick(provider)}
                                            className='w-full'
                                        >
                                            {isAuthenticating ? (
                                                <>
                                                    <Loader2 className='size-3.5 animate-spin' />
                                                    Authenticating...
                                                </>
                                            ) : (
                                                <>
                                                    <ProviderIcon className='size-4' />
                                                    {provider.charAt(0).toUpperCase() + provider.slice(1)}
                                                </>
                                            )}
                                        </Button>
                                    );
                                })}
                            </div>
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
}
