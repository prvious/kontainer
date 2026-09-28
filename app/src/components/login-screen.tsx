import { useEffect, useState } from "react";
import { getRouteApi, useNavigate, useRouter } from "@tanstack/react-router";
import { Container, Link as LinkIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { SessionIssue } from "@/components/session-issue";
import { AuthenticatingScreen } from "@/components/authenticating-screen";
import { useAuth } from "@/hooks/use-auth";
import { useSessionLoginHint } from "@/hooks/session/use-session-login-hint";
import { useAppVersion } from "@/hooks/use-app-version";

const routeApi = getRouteApi("/login");

function GitHubIcon({ className }: { className?: string }) {
    return (
        <svg viewBox='0 0 24 24' className={className} fill='currentColor'>
            <path d='M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-5.467 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z' />
        </svg>
    );
}

export function LoginScreen() {
    const { session, login, issue, retryCommit, retryRestore, logout, cancelLogin } = useAuth();
    const { data: hint } = useSessionLoginHint();
    const version = useAppVersion();
    const navigate = useNavigate();
    const router = useRouter();
    const { redirect: redirectTo } = routeApi.useSearch();
    const [originInput, setOriginInput] = useState<string | null>(null);
    const [authenticating, setAuthenticating] = useState(false);
    const origin = originInput ?? hint?.origin ?? "http://127.0.0.1:5150";

    useEffect(() => {
        if (!session) return;
        void router
            .invalidate()
            .then(() => navigate({ to: redirectTo || "/" }))
            .catch(() => {});
    }, [session, router, navigate, redirectTo]);

    async function handleConnect() {
        setAuthenticating(true);
        try {
            await login(origin.trim(), hint?.label ?? undefined);
        } catch {
            // The provider records a closed error code for SessionIssue.
        } finally {
            setAuthenticating(false);
        }
    }

    async function handleRetryCommit() {
        try {
            await retryCommit();
        } catch {
            // The provider records the new closed error code.
        }
    }

    if (authenticating) {
        return (
            <AuthenticatingScreen
                provider='GitHub'
                agentUrl={origin}
                step='auth'
                onCancel={() => {
                    void cancelLogin().catch(() => {});
                }}
            />
        );
    }

    return (
        <div className='flex flex-1 items-center justify-center'>
            <div className='flex w-[480px] flex-col gap-8 border border-border bg-card p-10'>
                <div className='flex flex-col items-center gap-3'>
                    <div className='flex size-14 items-center justify-center border border-primary bg-primary-soft'>
                        <Container className='size-7 text-primary' />
                    </div>
                    <span className='text-[28px] font-bold text-foreground'>kontainer</span>
                    <span className='font-mono text-xs font-medium text-secondary-foreground'>Connect to your controller</span>
                </div>

                <div className='flex flex-col gap-4'>
                    <label htmlFor='controller-origin' className='font-mono text-[10px] font-semibold tracking-[1px] text-muted-foreground'>
                        CONTROLLER URL
                    </label>
                    <div className='relative'>
                        <LinkIcon className='absolute top-1/2 left-3.5 size-4 -translate-y-1/2 text-muted-foreground' />
                        <Input
                            id='controller-origin'
                            type='url'
                            placeholder='https://controller.example'
                            value={origin}
                            onChange={(event) => setOriginInput(event.target.value)}
                            className='h-11 bg-muted pl-10 font-mono text-[13px]'
                        />
                    </div>
                    {issue && (
                        <SessionIssue
                            code={issue}
                            onRetryCommit={() => {
                                void handleRetryCommit();
                            }}
                            onRetryRestore={() => {
                                void retryRestore().catch(() => {});
                            }}
                            onLogout={() => {
                                void logout().catch(() => {});
                            }}
                        />
                    )}
                    <Button
                        type='button'
                        onClick={() => {
                            void handleConnect();
                        }}
                        className='h-11 w-full gap-2'
                    >
                        <GitHubIcon className='size-4' />
                        Connect with GitHub
                    </Button>
                </div>
                <p className='text-center font-mono text-[10px] text-muted-foreground'>kontainer desktop{version ? ` v${version}` : null}</p>
            </div>
        </div>
    );
}
