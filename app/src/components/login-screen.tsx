import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getRouteApi, useNavigate } from "@tanstack/react-router";
import { Container, ChevronRight, Link as LinkIcon, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useAuth } from "@/hooks/use-auth";
import { SetupWizard } from "@/components/setup-wizard";
import { AuthenticatingScreen } from "@/components/authenticating-screen";

const routeApi = getRouteApi("/login");

function GitHubIcon({ className }: { className?: string }) {
    return (
        <svg viewBox='0 0 24 24' className={className} fill='currentColor'>
            <path d='M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z' />
        </svg>
    );
}

export function LoginScreen() {
    const { login, profiles, switchProfile } = useAuth();
    const navigate = useNavigate();
    const { redirect: redirectTo } = routeApi.useSearch();
    const [agentUrl, setAgentUrl] = useState("http://localhost:3000");
    const [errorMessage, setErrorMessage] = useState<string | null>(null);
    const [isConnecting, setIsConnecting] = useState(false);
    const [authenticatingProvider, setAuthenticatingProvider] = useState<string | null>(null);
    const [setupRequired, setSetupRequired] = useState(false);

    function handleUrlChange(e: React.ChangeEvent<HTMLInputElement>) {
        setAgentUrl(e.target.value);
        setErrorMessage(null);
    }

    async function handleConnectWithGithub() {
        const url = agentUrl.replace(/\/+$/, "");
        setIsConnecting(true);
        setErrorMessage(null);

        // Step 1: Ping agent to check if it's reachable and if setup is needed
        try {
            const pingResult = await invoke<{ app: string; status: string }>("ping_agent", { url });
            if (pingResult.status === "setup_required") {
                setSetupRequired(true);
                setIsConnecting(false);
                return;
            }
        } catch (err) {
            setIsConnecting(false);
            setErrorMessage(typeof err === "string" ? err : "Could not reach the agent");
            return;
        }

        // Step 2: Start OAuth directly with GitHub (skip fetch_providers)
        setAuthenticatingProvider("github");
        setIsConnecting(false);

        try {
            await login(url, undefined, "github");
            navigate({ to: redirectTo || "/" });
        } catch (err) {
            setErrorMessage(typeof err === "string" ? err : err instanceof Error ? err.message : "Authentication failed");
            setAuthenticatingProvider(null);
        }
    }

    async function handleSavedConnection(profileId: string) {
        try {
            await switchProfile(profileId);
            navigate({ to: redirectTo || "/" });
        } catch (err) {
            setErrorMessage(typeof err === "string" ? err : err instanceof Error ? err.message : "Failed to connect to saved cluster");
        }
    }

    if (setupRequired) {
        return (
            <SetupWizard
                agentUrl={agentUrl.replace(/\/+$/, "")}
                onComplete={() => {
                    setSetupRequired(false);
                }}
                onBack={() => {
                    setSetupRequired(false);
                }}
            />
        );
    }

    if (authenticatingProvider) {
        return (
            <AuthenticatingScreen
                provider={authenticatingProvider}
                agentUrl={agentUrl.replace(/\/+$/, "")}
                step='auth'
                onCancel={() => setAuthenticatingProvider(null)}
            />
        );
    }

    const dotColors = ["bg-accent-green", "bg-accent-cyan", "bg-accent-orange", "bg-accent-purple", "bg-accent-red"];

    return (
        <div className='flex flex-1 items-center justify-center'>
            <div className='flex w-[480px] flex-col gap-8 border border-border bg-card p-10'>
                {/* Logo section */}
                <div className='flex flex-col items-center gap-3'>
                    <div className='flex size-14 items-center justify-center border border-accent-green bg-accent-green-20'>
                        <Container className='size-7 text-accent-green' />
                    </div>
                    <span className='text-[28px] font-bold text-foreground'>kontainer</span>
                    <span className='font-mono text-xs font-medium text-text-secondary'>Connect to your infrastructure agent</span>
                </div>

                {/* Agent URL + Connect with GitHub */}
                <div className='flex flex-col gap-4'>
                    <label className='font-mono text-[10px] font-semibold tracking-[1px] text-muted-foreground'>AGENT URL</label>
                    <div className='relative'>
                        <LinkIcon className='absolute top-1/2 left-3.5 size-4 -translate-y-1/2 text-muted-foreground' />
                        <Input
                            type='url'
                            placeholder='https://agent.kontainer.dev'
                            value={agentUrl}
                            onChange={handleUrlChange}
                            className='h-11 bg-bg-elevated pl-10 font-mono text-[13px]'
                        />
                    </div>

                    {errorMessage && <p className='font-mono text-xs text-accent-red'>{errorMessage}</p>}

                    <div className='flex items-center gap-3'>
                        <div className='h-px flex-1 bg-border' />
                        <span className='font-mono text-[10px] text-muted-foreground'>authenticate with</span>
                        <div className='h-px flex-1 bg-border' />
                    </div>

                    <Button type='button' disabled={isConnecting} onClick={handleConnectWithGithub} className='h-11 w-full gap-2'>
                        {isConnecting ? (
                            <>
                                <Loader2 className='size-4 animate-spin' />
                                Connecting...
                            </>
                        ) : (
                            <>
                                <GitHubIcon className='size-4' />
                                Connect with GitHub
                            </>
                        )}
                    </Button>
                </div>

                {/* Saved connections */}
                {profiles.length > 0 && (
                    <div className='flex flex-col gap-3'>
                        <div className='flex items-center justify-between'>
                            <span className='font-mono text-[10px] font-semibold tracking-[1px] text-muted-foreground'>SAVED CONNECTIONS</span>
                            <span className='rounded bg-bg-elevated px-2 py-0.5 font-mono text-[10px] text-muted-foreground'>
                                {profiles.length} AGENT{profiles.length !== 1 ? "S" : ""}
                            </span>
                        </div>
                        <div className='overflow-hidden border border-border bg-bg-elevated'>
                            {profiles.map((profile, i) => (
                                <button
                                    key={profile.id}
                                    onClick={() => handleSavedConnection(profile.id)}
                                    className='flex w-full items-center gap-3 border-b border-border px-3.5 py-3 text-left last:border-b-0 hover:bg-bg-subtle'
                                >
                                    <span className={`size-2 shrink-0 rounded-full ${dotColors[i % dotColors.length]}`} />
                                    <div className='flex flex-1 flex-col gap-0.5 overflow-hidden'>
                                        <span className='truncate text-sm font-semibold text-foreground'>{profile.label}</span>
                                        <span className='truncate font-mono text-[11px] text-muted-foreground'>{profile.agentUrl}</span>
                                    </div>
                                    <ChevronRight className='size-3.5 shrink-0 text-muted-foreground' />
                                </button>
                            ))}
                        </div>
                    </div>
                )}

                {/* Version footer */}
                <p className='text-center font-mono text-[10px] text-muted-foreground'>kontainer desktop v1.0.0</p>
            </div>
        </div>
    );
}
