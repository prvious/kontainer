import { Loader2, Check, Globe, Github, Key } from "lucide-react";
import { Button } from "@/components/ui/button";

type AuthStep = "browser" | "auth" | "session";

export function AuthenticatingScreen({ provider, agentUrl, step, onCancel }: { provider: string; agentUrl: string; step: AuthStep; onCancel: () => void }) {
    const providerLabel = provider.charAt(0).toUpperCase() + provider.slice(1);

    const steps: { id: AuthStep; label: string; icon: typeof Globe }[] = [
        { id: "browser", label: "Browser Opened", icon: Globe },
        { id: "auth", label: `${providerLabel} Auth`, icon: Github },
        { id: "session", label: "Saving Session", icon: Key },
    ];

    const currentIdx = steps.findIndex((s) => s.id === step);

    return (
        <div className='flex flex-1 items-center justify-center'>
            <div className='flex w-[500px] flex-col items-center gap-6 border border-border bg-card p-10'>
                {/* Loader */}
                <Loader2 className='size-8 animate-spin text-primary' />

                {/* Title */}
                <div className='flex flex-col items-center gap-2'>
                    <h1 className='text-xl font-bold text-foreground'>Authenticating...</h1>
                    <p className='font-mono text-[13px] text-secondary-foreground'>Waiting for {providerLabel} authorization</p>
                </div>

                {/* Steps */}
                <div className='flex w-full items-center justify-center gap-6'>
                    {steps.map((s, i) => {
                        const isCompleted = i < currentIdx;
                        const isCurrent = i === currentIdx;
                        const Icon = isCompleted ? Check : s.icon;

                        return (
                            <div key={s.id} className='flex flex-col items-center gap-2'>
                                <Icon
                                    className={`size-5 ${isCompleted ? "text-primary" : isCurrent ? 'animate-pulse text-warning' : "text-muted-foreground"}`}
                                />
                                <span
                                    className={`font-mono text-[10px] font-semibold ${
                                        isCompleted ? "text-primary" : isCurrent ? "text-warning" : "text-muted-foreground"
                                    }`}
                                >
                                    {s.label}
                                </span>
                            </div>
                        );
                    })}
                </div>

                {/* Info text */}
                <p className='text-center font-mono text-[11px] leading-relaxed text-muted-foreground'>
                    A browser window has been opened. Complete the authorization to continue.
                </p>

                {/* Cancel */}
                <Button variant='outline' size='sm' onClick={onCancel}>
                    Cancel
                </Button>

                {/* Agent URL */}
                <p className='font-mono text-[10px] text-muted-foreground'>Connecting to controller {agentUrl}</p>
            </div>
        </div>
    );
}
