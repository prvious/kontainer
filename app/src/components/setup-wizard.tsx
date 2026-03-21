import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Building2, ExternalLink, Github, Key, Loader2, Tag } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface SetupWizardProps {
    agentUrl: string;
    onComplete: () => void;
    onBack: () => void;
}

type SetupStep = "token" | "app" | "done";

export function SetupWizard({ agentUrl, onComplete, onBack }: SetupWizardProps) {
    const [setupToken, setSetupToken] = useState("");
    const [orgName, setOrgName] = useState("");
    const [error, setError] = useState<string | null>(null);
    const [isLoading, setIsLoading] = useState(false);
    const [activeStep, setActiveStep] = useState<SetupStep>("token");
    const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null);

    useEffect(() => {
        return () => {
            if (pollingRef.current) clearInterval(pollingRef.current);
        };
    }, []);

    async function handleValidateAndStart() {
        setError(null);
        setIsLoading(true);

        // Step 1: Verify setup token
        try {
            const result = await invoke<{ step: string }>("setup_verify_token", {
                url: agentUrl,
                token: setupToken,
            });

            if (result.step === "github_connected") {
                // Already connected — complete setup
                setActiveStep("done");
                try {
                    await invoke("setup_complete", { url: agentUrl, token: setupToken });
                    setIsLoading(false);
                    onComplete();
                } catch (completeErr) {
                    setIsLoading(false);
                    setError(typeof completeErr === "string" ? completeErr : "Failed to complete setup");
                    setActiveStep("token");
                }
                return;
            }
        } catch (err) {
            setIsLoading(false);
            setError(typeof err === "string" ? err : "Invalid setup token");
            return;
        }

        // Step 2: Start GitHub App manifest flow
        setActiveStep("app");
        try {
            const result = await invoke<{ setupUrl: string }>("setup_github_connect", {
                url: agentUrl,
                token: setupToken,
                org: orgName.trim() || null,
            });

            await openUrl(result.setupUrl);

            // Step 3: Poll for completion
            let consecutiveFailures = 0;

            pollingRef.current = setInterval(async () => {
                try {
                    const status = await invoke<{ step: string }>("setup_verify_token", {
                        url: agentUrl,
                        token: setupToken,
                    });
                    consecutiveFailures = 0;
                    if (status.step === "github_connected") {
                        if (pollingRef.current) clearInterval(pollingRef.current);
                        pollingRef.current = null;
                        setActiveStep("done");

                        try {
                            await invoke("setup_complete", { url: agentUrl, token: setupToken });
                            setIsLoading(false);
                            onComplete();
                        } catch (err) {
                            setIsLoading(false);
                            setError(typeof err === "string" ? err : "Failed to complete setup");
                            setActiveStep("token");
                        }
                    }
                } catch (err) {
                    consecutiveFailures++;
                    if (consecutiveFailures >= 5) {
                        if (pollingRef.current) clearInterval(pollingRef.current);
                        pollingRef.current = null;
                        setIsLoading(false);
                        setActiveStep("token");
                        setError(typeof err === "string" ? err : "Lost connection to agent");
                    }
                }
            }, 2000);
        } catch (err) {
            setIsLoading(false);
            setActiveStep("token");
            setError(typeof err === "string" ? err : "Failed to start GitHub setup");
        }
    }

    const steps: { id: SetupStep; label: string; icon: typeof Key }[] = [
        { id: "token", label: "Setup Token", icon: Key },
        { id: "app", label: "Create App", icon: Github },
        { id: "done", label: "Retrieve Token", icon: Tag },
    ];

    const currentIdx = steps.findIndex((s) => s.id === activeStep);

    return (
        <div className='flex flex-1 items-center justify-center'>
            <div className='flex w-[560px] flex-col gap-7 border border-border bg-card px-10 pt-10 pb-8'>
                {/* Badge */}
                <span className='w-fit rounded border border-accent-orange bg-accent-orange-20 px-2.5 py-1 font-mono text-[10px] font-semibold text-accent-orange'>
                    FIRST-TIME SETUP
                </span>

                {/* Title */}
                <h1 className='text-[22px] font-bold text-foreground'>Create your GitHub App manifest</h1>

                {/* Description */}
                <p className='font-mono text-xs leading-relaxed font-medium text-text-secondary'>
                    Kontainer will open your browser to GitHub so you can create the app in your organization. Before that, you must confirm the agent-generated
                    setup token so nobody with the URL can hijack the cluster.
                </p>

                {/* Inputs */}
                <div className='flex flex-col gap-3.5'>
                    <label className='font-mono text-[10px] font-semibold tracking-[1px] text-muted-foreground'>GITHUB ORGANIZATION</label>
                    <div className='relative'>
                        <Building2 className='absolute top-1/2 left-4 size-4 -translate-y-1/2 text-muted-foreground' />
                        <Input
                            type='text'
                            placeholder='Leave blank for personal account'
                            value={orgName}
                            onChange={(e) => setOrgName(e.target.value)}
                            disabled={isLoading}
                            className='h-11 bg-bg-elevated pl-11 font-mono text-[13px]'
                        />
                    </div>

                    <label className='font-mono text-[10px] font-semibold tracking-[1px] text-muted-foreground'>SETUP TOKEN</label>
                    <div className='relative'>
                        <Key className='absolute top-1/2 left-4 size-4 -translate-y-1/2 text-muted-foreground' />
                        <Input
                            type='text'
                            placeholder='kst_live_29f0-8d0z-1d7c'
                            value={setupToken}
                            onChange={(e) => setSetupToken(e.target.value)}
                            disabled={isLoading}
                            className='h-11 bg-bg-elevated pl-11 font-mono text-[13px]'
                        />
                    </div>
                </div>

                {/* Step indicators */}
                <div className='flex items-center justify-center gap-6'>
                    {steps.map((s, i) => {
                        const isCompleted = i < currentIdx;
                        const isCurrent = i === currentIdx;
                        const Icon = s.icon;

                        return (
                            <div key={s.id} className='flex flex-col items-center gap-2'>
                                <Icon
                                    className={`size-5 ${
                                        isCompleted ? "text-accent-green" : isCurrent ? 'animate-pulse text-accent-orange' : "text-muted-foreground"
                                    }`}
                                />
                                <span
                                    className={`font-mono text-[10px] font-semibold ${
                                        isCompleted ? "text-accent-green" : isCurrent ? "text-accent-orange" : "text-muted-foreground"
                                    }`}
                                >
                                    {s.label}
                                </span>
                            </div>
                        );
                    })}
                </div>

                {/* Helper text */}
                <p className='text-center font-mono text-[11px] leading-relaxed text-muted-foreground'>
                    Use the one-time setup token generated by the agent, then choose a unique app name for your org. GitHub will ask you to confirm permissions
                    before redirecting back to the agent.
                </p>

                {/* Error */}
                {error && <p className='text-center font-mono text-xs text-accent-red'>{error}</p>}

                {/* Actions */}
                <div className='flex items-center justify-center gap-3'>
                    <Button variant='outline' onClick={onBack} disabled={isLoading} className='h-10 px-4'>
                        Cancel
                    </Button>
                    <Button disabled={isLoading || !setupToken.trim()} onClick={handleValidateAndStart} className='h-10 gap-2 px-4'>
                        {isLoading ? (
                            <>
                                <Loader2 className='size-3.5 animate-spin' />
                                {activeStep === "app" ? "Waiting for GitHub..." : "Validating..."}
                            </>
                        ) : (
                            <>
                                <ExternalLink className='size-3.5' />
                                Validate Token & Start
                            </>
                        )}
                    </Button>
                </div>

                {/* Footer */}
                <p className='text-center font-mono text-[10px] text-muted-foreground'>Manifest setup via {agentUrl}/github/app-manifest</p>
            </div>
        </div>
    );
}
