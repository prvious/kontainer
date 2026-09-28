import { useRef, useState, type FormEvent } from "react";
import { Activity, Link as LinkIcon, LockKeyhole, LogIn, Mail, RefreshCw, User, UserPlus } from "lucide-react";
import { ActionButton } from "@/components/kit/action-button";
import { notify } from "@/components/kit/notify";
import { LogoMark } from "@/components/logo";
import { SessionIssue } from "@/components/session-issue";
import { ConnectField } from "@/components/connect/connect-field";
import { SavedConnections } from "@/components/connect/saved-connections";
import {
    DEFAULT_ORIGIN,
    connectionHost,
    diagnosticSteps,
    emailError,
    invalidCredentialsMessage,
    lockedMessage,
    nameError,
    originError,
    passwordError,
    serverFieldErrors,
    unreachableMessage,
    type ConnectErrors,
    type ConnectField as FieldName,
} from "@/components/connect/connect-rules";
import { useAuth } from "@/hooks/use-auth";
import { useAppVersion } from "@/hooks/use-app-version";
import { useSessionBootstrap } from "@/hooks/session/use-session-bootstrap";
import { useConnectionProbe, useSessionConnections } from "@/hooks/session/use-session-connections";
import { isLoginFailure, type LoginFailure, type SavedConnection } from "@/types/session";
import { cn } from "@/lib/utils";

const CUSTODY_ISSUES = new Set([
    "vault_unavailable",
    "vault_write_failed",
    "local_state_failed",
    "cleanup_uncertain",
    "profile_conflict",
    "revoke_failed",
    "local_cleanup_failed",
]);

/**
 * The Connect card (`wwb6d` sign in, `wLzP9` create owner) with states 02.S2 (agent unreachable)
 * and 02.S7 (invalid credentials). The bootstrap check on the URL picks the variant.
 */
export function ConnectCard({ initialOrigin }: { initialOrigin?: string }) {
    const auth = useAuth();
    const version = useAppVersion();
    const connections = useSessionConnections();
    const emailRef = useRef<HTMLInputElement>(null);

    const [originInput, setOriginInput] = useState<string | null>(null);
    const [committedOrigin, setCommittedOrigin] = useState<string | null>(null);
    const [displayName, setDisplayName] = useState("");
    const [email, setEmail] = useState("");
    const [password, setPassword] = useState("");
    const [confirm, setConfirm] = useState("");
    const [errors, setErrors] = useState<ConnectErrors>({});
    const [notice, setNotice] = useState<string | null>(null);
    const [submitting, setSubmitting] = useState(false);

    // Prefill: the origin passed in (switcher, settings), else the most recent connection.
    const waitingForConnections = initialOrigin === undefined && connections.isPending;
    const defaultOrigin = initialOrigin ?? connections.data?.[0]?.origin ?? DEFAULT_ORIGIN;
    const originText = originInput ?? (waitingForConnections ? "" : defaultOrigin);
    const candidate = committedOrigin ?? (originInput === null && !waitingForConnections ? defaultOrigin : null);
    const checkedOrigin = candidate !== null && !originError(candidate) ? candidate.trim() : null;

    const bootstrap = useSessionBootstrap(checkedOrigin);
    const bootstrapFailure = isLoginFailure(bootstrap.error) ? bootstrap.error : null;
    const unreachable = bootstrapFailure?.code === "controller_unavailable";
    const probe = useConnectionProbe(unreachable ? checkedOrigin : null);
    const variant = bootstrap.data === false ? "create" : "signin";
    const probing = bootstrap.isFetching;
    const locked = probing || submitting;

    const urlError =
        errors.origin ??
        (unreachable && checkedOrigin
            ? unreachableMessage(checkedOrigin, probe.data)
            : bootstrapFailure?.code === "invalid_response"
              ? "This URL answered, but not like a Kontainer agent."
              : undefined);

    function setFieldError(field: FieldName, message: string | undefined) {
        setErrors((current) => ({ ...current, [field]: message }));
    }

    function commitOrigin() {
        const origin = originText.trim();
        const problem = originError(origin);
        setFieldError("origin", problem);
        setNotice(null);
        if (!problem) setCommittedOrigin(origin);
    }

    function choose(connection: SavedConnection) {
        setOriginInput(connection.origin);
        setCommittedOrigin(connection.origin);
        setErrors({});
        setNotice(null);
        emailRef.current?.focus();
    }

    function validate(): ConnectErrors {
        const next: ConnectErrors = { origin: originError(originText), email: emailError(email), password: passwordError(password) };
        if (variant === "create") {
            next.displayName = nameError(displayName);
            next.confirm = confirm === password ? undefined : "The passwords don't match.";
        }
        return next;
    }

    function handleFailure(failure: LoginFailure, origin: string) {
        switch (failure.code) {
            case "invalid_credentials":
                setPassword("");
                setFieldError("password", invalidCredentialsMessage(failure));
                return;
            case "login_locked":
                setFieldError("password", lockedMessage(failure, Date.now()));
                return;
            case "validation_failed":
                setErrors(serverFieldErrors(failure.fields));
                return;
            case "owner_exists":
                setNotice("This agent already has its owner. Sign in with that account instead.");
                void bootstrap.refetch();
                return;
            case "invalid_origin":
                setFieldError("origin", "This URL isn't a valid agent address.");
                return;
        }
        if (CUSTODY_ISSUES.has(failure.code)) return; // The provider shows SessionIssue.
        if (failure.code === "controller_unavailable") void bootstrap.refetch();
        notify.error(`Couldn't sign in to ${connectionHost(origin)}`, {
            code: failure.code,
            message: failure.code === "controller_unavailable" ? "The agent didn't answer. The typed URL is kept." : "The agent returned an unexpected answer.",
            requestId: failure.requestId,
        });
    }

    async function submit(event: FormEvent) {
        event.preventDefault();
        if (locked) return;
        const origin = originText.trim();
        if (unreachable && origin === checkedOrigin) {
            void bootstrap.refetch();
            void probe.refetch();
            return;
        }
        const next = validate();
        setErrors(next);
        setNotice(null);
        if (Object.values(next).some(Boolean)) return;
        if (origin !== checkedOrigin) setCommittedOrigin(origin);
        const label = connections.data?.find((connection) => connection.origin === origin)?.label ?? null;
        setSubmitting(true);
        try {
            if (variant === "create") await auth.register({ origin, displayName: displayName.trim(), email: email.trim(), password, label });
            else await auth.login({ origin, email: email.trim(), password, label });
            setPassword("");
            setConfirm("");
        } catch (error) {
            if (isLoginFailure(error)) handleFailure(error, origin);
        } finally {
            setSubmitting(false);
        }
    }

    const steps = unreachable && checkedOrigin ? diagnosticSteps(checkedOrigin, probe.data) : null;
    const buttonLabel = locked ? "CONNECTING…" : unreachable ? "TRY AGAIN" : variant === "create" ? "Create owner account" : "Sign in";
    const buttonIcon = unreachable ? RefreshCw : variant === "create" ? UserPlus : LogIn;

    return (
        <div className='flex flex-1 items-start justify-center overflow-y-auto py-10'>
            <form noValidate onSubmit={(event) => void submit(event)} className='flex w-120 flex-col gap-4 rounded-lg border border-border bg-card p-8'>
                <header className='flex flex-col items-center gap-3 pb-2'>
                    {variant === "create" && (
                        <span className='rounded-sm border border-success bg-success-soft px-2.5 py-1 font-mono text-3xs font-bold tracking-[0.5px] text-success uppercase'>
                            First-time setup
                        </span>
                    )}
                    <div className='flex size-14 items-center justify-center rounded-lg border border-border bg-secondary'>
                        <LogoMark size='lg' className='size-7' />
                    </div>
                    <h1 className='font-brand text-2xl font-black tracking-[0.5px] text-foreground'>KONTAINER</h1>
                    <p className='font-mono text-xs text-secondary-foreground'>
                        {variant === "create" ? "First run — create the owner account" : "Sign in to your infrastructure agent"}
                    </p>
                </header>

                <ConnectField
                    id='agent-url'
                    label='Agent URL'
                    icon={LinkIcon}
                    type='url'
                    autoComplete='url'
                    placeholder='https://agent.example.com'
                    value={originText}
                    readOnly={locked}
                    aria-readonly={locked || undefined}
                    error={urlError}
                    onChange={(event) => setOriginInput(event.target.value)}
                    onBlur={commitOrigin}
                />
                {steps && (
                    <div
                        aria-label='Connection diagnostics'
                        className='flex items-center gap-4 rounded-sm border border-border bg-muted px-2.5 py-1.5 font-mono text-3xs text-muted-foreground'
                    >
                        <Activity aria-hidden className='size-3' />
                        {steps.map((step, index) => (
                            <span
                                key={step.label}
                                data-state={step.state}
                                className={cn(step.state === "failed" && "font-semibold text-destructive", step.state === "ok" && "text-secondary-foreground")}
                            >
                                {index > 0 && <span className='mr-2 text-foreground-faint'>→</span>}
                                {step.label} {step.state === "ok" ? "✓" : step.state === "failed" ? "✗" : "—"}
                            </span>
                        ))}
                    </div>
                )}

                {variant === "create" && (
                    <ConnectField
                        id='owner-name'
                        label='Name'
                        icon={User}
                        autoComplete='name'
                        value={displayName}
                        error={errors.displayName}
                        onChange={(event) => setDisplayName(event.target.value)}
                        onBlur={() => setFieldError("displayName", nameError(displayName))}
                    />
                )}
                <ConnectField
                    ref={emailRef}
                    id='owner-email'
                    label='Email'
                    icon={Mail}
                    type='email'
                    autoComplete='username'
                    value={email}
                    error={errors.email}
                    onChange={(event) => setEmail(event.target.value)}
                    onBlur={() => email && setFieldError("email", emailError(email))}
                />
                <ConnectField
                    id='owner-password'
                    label='Password'
                    icon={LockKeyhole}
                    secret
                    autoComplete={variant === "create" ? "new-password" : "current-password"}
                    placeholder='Password'
                    value={password}
                    error={errors.password}
                    helper={variant === "create" ? "At least 12 characters. There is no email verification — keep it somewhere safe." : undefined}
                    onChange={(event) => setPassword(event.target.value)}
                    onBlur={() => password && setFieldError("password", passwordError(password))}
                />
                {variant === "create" && (
                    <ConnectField
                        id='owner-password-confirm'
                        label='Confirm password'
                        icon={LockKeyhole}
                        secret
                        autoComplete='new-password'
                        value={confirm}
                        error={errors.confirm}
                        onChange={(event) => setConfirm(event.target.value)}
                        onBlur={() => confirm && setFieldError("confirm", confirm === password ? undefined : "The passwords don't match.")}
                    />
                )}

                {notice && <p className='font-mono text-2xs text-warning'>{notice}</p>}
                {auth.issue && CUSTODY_ISSUES.has(auth.issue) && (
                    <SessionIssue
                        code={auth.issue}
                        onRetryCommit={() => void auth.retryCommit().catch(() => {})}
                        onRetryRestore={() => void auth.retryRestore().catch(() => {})}
                        onLogout={() => void auth.logout().catch(() => {})}
                    />
                )}

                <ActionButton type='submit' variant='primary' size='md' icon={buttonIcon} pending={locked} className='h-11 w-full normal-case'>
                    {buttonLabel}
                </ActionButton>
                <p className='text-center font-mono text-3xs text-muted-foreground'>
                    {variant === "create"
                        ? "This account becomes the Owner. Registration closes once it exists."
                        : "Forgot the password? Reset it from the controller CLI."}
                </p>

                {variant === "signin" && connections.data && <SavedConnections connections={connections.data} onChoose={choose} />}

                <p className='pt-2 text-center font-mono text-3xs text-muted-foreground'>kontainer desktop{version ? ` v${version}` : null}</p>
            </form>
        </div>
    );
}
