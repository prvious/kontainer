import { useEffect, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Logo } from "@/components/logo";
import { useAuth } from "@/hooks/use-auth";
import { Route } from "@/routes/login";
import { Field, FieldError, FieldGroup, FieldLabel, FieldLegend, FieldSet } from "@/components/ui/field";

export function LoginScreen() {
    const { user, login } = useAuth();
    const navigate = useNavigate();
    const { redirect } = Route.useSearch();
    const [agentUrl, setAgentUrl] = useState("http://localhost:8080");
    const [isPinging, setIsPinging] = useState(false);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        if (user) {
            navigate({ to: redirect || "/" });
        }
    }, [user, redirect, navigate]);

    async function handleSubmit(e: React.FormEvent) {
        e.preventDefault();
        const url = agentUrl.replace(/\/+$/, "");

        setError(null);
        setIsPinging(true);

        try {
            await invoke("ping_agent", { url });
        } catch (err) {
            setError(typeof err === "string" ? err : "Could not reach the agent");
            setIsPinging(false);
            return;
        }

        setIsPinging(false);
        login(url);
    }

    return (
        <div className="flex min-h-screen flex-col items-center justify-center px-4">
            <form onSubmit={handleSubmit} className="-mt-20 flex w-full max-w-sm flex-col items-center gap-8">
                <Logo />
                <Card className="w-full">
                    <CardHeader>
                        <CardTitle>Connect to agent</CardTitle>
                        <CardDescription>Enter your agent's URL to get started.</CardDescription>
                    </CardHeader>
                    <CardContent className="">
                        <FieldGroup>
                            <FieldSet className="gap-5">
                                <Field>
                                    <FieldLabel htmlFor="agent-url">Agent URL</FieldLabel>
                                    <Input
                                        id="agent-url"
                                        type="url"
                                        placeholder="http://localhost:8080"
                                        value={agentUrl}
                                        onChange={(e) => setAgentUrl(e.target.value)}
                                        required
                                    />
                                    {error && <FieldError>{error}</FieldError>}
                                </Field>
                                <Field>
                                    <Button type="submit" disabled={isPinging}>
                                        {isPinging ? "Connecting..." : "Connect"}
                                    </Button>
                                </Field>
                            </FieldSet>
                        </FieldGroup>
                    </CardContent>
                </Card>
            </form>
            <p className="absolute bottom-6 text-xs text-muted-foreground">
                Built by{" "}
                <a
                    href="https://prvious.dev"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="underline underline-offset-2 hover:text-foreground"
                >
                    prvious.dev
                </a>
            </p>
        </div>
    );
}
