import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowLeft, CheckCircle2, Github, Key, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

interface SetupWizardProps {
    agentUrl: string;
    onComplete: () => void;
    onBack: () => void;
}

type Step = "token" | "github" | "confirm";

const STEPS: Step[] = ["token", "github", "confirm"];

export function SetupWizard({ agentUrl, onComplete, onBack }: SetupWizardProps) {
    const [step, setStep] = useState<Step>("token");
    const [setupToken, setSetupToken] = useState("");
    const [error, setError] = useState<string | null>(null);
    const [isLoading, setIsLoading] = useState(false);
    const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null);

    useEffect(() => {
        return () => {
            if (pollingRef.current) clearInterval(pollingRef.current);
        };
    }, []);

    async function handleVerifyToken(e: React.FormEvent) {
        e.preventDefault();
        setError(null);
        setIsLoading(true);

        try {
            const result = await invoke<{ step: string }>("setup_verify_token", {
                url: agentUrl,
                token: setupToken,
            });

            if (result.step === "github_connected") {
                setStep("confirm");
            } else {
                setStep("github");
            }
        } catch (err) {
            setError(typeof err === "string" ? err : "Failed to verify token");
        } finally {
            setIsLoading(false);
        }
    }

    async function handleConnectGithub() {
        setError(null);
        setIsLoading(true);

        try {
            const result = await invoke<{ setupUrl: string }>("setup_github_connect", {
                url: agentUrl,
                token: setupToken,
            });

            await openUrl(result.setupUrl);

            pollingRef.current = setInterval(async () => {
                try {
                    const status = await invoke<{ step: string }>("setup_verify_token", {
                        url: agentUrl,
                        token: setupToken,
                    });
                    if (status.step === "github_connected") {
                        if (pollingRef.current) clearInterval(pollingRef.current);
                        pollingRef.current = null;
                        setIsLoading(false);
                        setStep("confirm");
                    }
                } catch {
                    // Keep polling
                }
            }, 2000);
        } catch (err) {
            setIsLoading(false);
            setError(typeof err === "string" ? err : "Failed to start GitHub setup");
        }
    }

    async function handleComplete() {
        setError(null);
        setIsLoading(true);

        try {
            await invoke("setup_complete", {
                url: agentUrl,
                token: setupToken,
            });
            onComplete();
        } catch (err) {
            setIsLoading(false);
            setError(typeof err === "string" ? err : "Failed to complete setup");
        }
    }

    const currentStepIndex = STEPS.indexOf(step);

    return (
        <div className='flex h-screen p-2 pt-10'>
            <div data-tauri-drag-region className='grain flex flex-1 flex-col items-center justify-center overflow-hidden rounded-xl bg-background shadow-sm'>
                <div className='w-full max-w-sm px-6'>
                    <button type='button' onClick={onBack} className='mb-4 flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground'>
                        <ArrowLeft className='size-3' />
                        Back
                    </button>

                    <h1 className='text-lg font-semibold'>Agent Setup</h1>
                    <p className='mt-1 text-sm text-muted-foreground'>{agentUrl}</p>

                    {/* Step indicators */}
                    <div className='mt-4 flex items-center gap-2'>
                        {STEPS.map((s, i) => (
                            <div key={s} className='flex items-center gap-2'>
                                {i > 0 && <div className='h-px w-4 bg-border' />}
                                <div
                                    className={`flex size-6 items-center justify-center rounded-full text-xs font-medium ${
                                        step === s
                                            ? 'bg-primary text-primary-foreground'
                                            : currentStepIndex > i
                                              ? 'bg-primary/20 text-primary'
                                              : 'bg-muted text-muted-foreground'
                                    }`}
                                >
                                    {currentStepIndex > i ? <CheckCircle2 className='size-3.5' /> : i + 1}
                                </div>
                            </div>
                        ))}
                    </div>

                    {/* Step 1: Token */}
                    {step === "token" && (
                        <form onSubmit={handleVerifyToken} className='mt-6'>
                            <p className='mb-4 text-sm text-muted-foreground'>Enter the setup token shown in your agent's logs.</p>
                            <Label htmlFor='setup-token' className='mb-2'>
                                Setup Token
                            </Label>
                            <div className='relative'>
                                <Input
                                    id='setup-token'
                                    type='text'
                                    placeholder='ktsetup_...'
                                    value={setupToken}
                                    onChange={(e) => setSetupToken(e.target.value)}
                                    required
                                    className='h-9 pr-9 font-mono text-sm'
                                />
                                <div className='absolute top-1/2 right-3 -translate-y-1/2'>
                                    <Key className='size-3.5 text-muted-foreground' />
                                </div>
                            </div>
                            {error && <p className='mt-2 text-xs text-destructive'>{error}</p>}
                            <Button type='submit' disabled={isLoading} size='sm' className='mt-3 w-full'>
                                {isLoading ? (
                                    <>
                                        <Loader2 className='size-3.5 animate-spin' />
                                        Verifying...
                                    </>
                                ) : (
                                    "Verify"
                                )}
                            </Button>
                        </form>
                    )}

                    {/* Step 2: GitHub */}
                    {step === "github" && (
                        <div className='mt-6'>
                            <p className='mb-4 text-sm text-muted-foreground'>Connect a GitHub organization to manage access to this agent.</p>
                            {error && <p className='mb-2 text-xs text-destructive'>{error}</p>}
                            <Button type='button' variant='outline' size='sm' disabled={isLoading} onClick={handleConnectGithub} className='w-full'>
                                {isLoading ? (
                                    <>
                                        <Loader2 className='size-3.5 animate-spin' />
                                        Waiting for GitHub...
                                    </>
                                ) : (
                                    <>
                                        <Github className='size-4' />
                                        Connect GitHub
                                    </>
                                )}
                            </Button>
                            {isLoading && <p className='mt-2 text-xs text-muted-foreground'>Complete the setup in your browser, then return here.</p>}
                        </div>
                    )}

                    {/* Step 3: Confirm */}
                    {step === "confirm" && (
                        <div className='mt-6'>
                            <div className='flex items-center gap-2 rounded-md border border-border bg-muted/50 p-3'>
                                <CheckCircle2 className='size-4 text-success' />
                                <span className='text-sm'>GitHub App connected</span>
                            </div>
                            <p className='mt-3 text-sm text-muted-foreground'>All members of the connected organization will be able to log in.</p>
                            {error && <p className='mt-2 text-xs text-destructive'>{error}</p>}
                            <Button type='button' size='sm' disabled={isLoading} onClick={handleComplete} className='mt-4 w-full'>
                                {isLoading ? (
                                    <>
                                        <Loader2 className='size-3.5 animate-spin' />
                                        Completing setup...
                                    </>
                                ) : (
                                    "Complete Setup"
                                )}
                            </Button>
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
}
