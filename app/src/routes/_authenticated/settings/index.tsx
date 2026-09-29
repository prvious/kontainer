import { useRef, useState } from "react";
import { createFileRoute, useBlocker } from "@tanstack/react-router";
import { ArrowLeftRight, ExternalLink, LogIn, LogOut, Monitor, Plug, Server } from "lucide-react";
import { PageHeader } from "@/components/page-header";
import { ActionButton } from "@/components/kit/action-button";
import { ConfirmDialog } from "@/components/kit/confirm-dialog";
import { fieldHelperId } from "@/components/kit/field-ids";
import { notify } from "@/components/kit/notify";
import { LabelTooltip } from "@/components/kit/tooltips";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { uptime } from "@/components/containers/container-format";
import { FontSizeStepper } from "@/components/settings/font-size-stepper";
import { PendingChangesBar } from "@/components/settings/pending-changes-bar";
import { SettingsRow, SettingsSection } from "@/components/settings/settings-section";
import {
    REFRESH_INTERVALS,
    RETENTION_DAYS,
    ROW_LABELS,
    SCROLLBACKS,
    SHELLS,
    THEMES,
    appSettingsFrom,
    changedRows,
    days,
    draftFrom,
    fieldErrors,
    seconds,
    valueText,
    withCurrent,
    type SettingsDraft,
    type SettingsRow as Row,
    type ShellChoice,
} from "@/components/settings/settings-form";
import { useAppSettings, useSaveAppSettings } from "@/hooks/use-app-settings";
import { useAuth } from "@/hooks/use-auth";
import { useOpenShells } from "@/hooks/use-exec-sessions";
import { useOpenLogTabs } from "@/hooks/use-log-tabs";
import { useControllerSettings, useControllerStatus, useUpdateControllerSettings } from "@/hooks/use-nodes";
import { useNow } from "@/hooks/use-now";
import { useSwitchAgent } from "@/hooks/use-switch-agent";
import type { AppSettings, ColorScheme } from "@/types/settings";
import { cn } from "@/lib/utils";

export const Route = createFileRoute("/_authenticated/settings/")({
    component: SettingsPage,
});

const APPLICATION_ROWS: Row[] = ["scheme", "refreshInterval", "shell", "fontSize", "scrollback"];
const plural = (count: number, word: string) => `${count} ${word}${count === 1 ? "" : "s"}`;
const READ_ONLY = "h-8 w-56 cursor-default border-transparent bg-muted font-mono text-xs text-secondary-foreground focus-visible:ring-0";

function ChoiceSelect({
    id,
    value,
    options,
    changed,
    disabled,
    onChange,
}: {
    id: string;
    value: string;
    options: { value: string; label: string }[];
    changed: boolean;
    disabled?: boolean;
    onChange: (value: string) => void;
}) {
    return (
        <Select value={value} onValueChange={(next) => typeof next === "string" && onChange(next)} items={options} disabled={disabled}>
            <SelectTrigger id={id} className={cn("h-8 min-w-24 font-mono text-xs", changed && "border-primary")}>
                <SelectValue />
            </SelectTrigger>
            <SelectContent>
                {options.map((option) => (
                    <SelectItem key={option.value} value={option.value} className='font-mono text-xs'>
                        {option.label}
                    </SelectItem>
                ))}
            </SelectContent>
        </Select>
    );
}

/** The controller's uptime from its start time, on the shared clock. */
function Uptime({ id, startedAt }: { id: string; startedAt: string | undefined }) {
    const now = useNow();
    return <Input id={id} readOnly value={startedAt ? uptime((now - Date.parse(startedAt)) / 1_000) : "—"} className={READ_ONLY} />;
}

function SettingsPage() {
    const { session } = useAuth();
    const appSettings = useAppSettings();
    const controllerSettings = useControllerSettings();
    const status = useControllerStatus();
    const saveApp = useSaveAppSettings();
    const saveRetention = useUpdateControllerSettings();
    const switchAgent = useSwitchAgent();
    const terminals = useOpenShells();
    const logStreams = useOpenLogTabs();

    const [edits, setEdits] = useState<Partial<SettingsDraft>>({});
    const [touched, setTouched] = useState<ReadonlySet<Row>>(new Set());
    const [switchTo, setSwitchTo] = useState<string | null>(null);
    // Set while switching agents, so signing out and landing on Connect isn't blocked.
    const leaving = useRef(false);

    const stored = appSettings.data;
    const baseline = draftFrom(stored ?? ({} as AppSettings), session?.origin ?? "", controllerSettings.data?.metricsRetentionDays ?? null);
    const draft: SettingsDraft = { ...baseline, ...edits };
    const changed = stored ? changedRows(baseline, draft) : [];
    const errors = fieldErrors(draft);
    const invalid = changed.filter((row) => errors[row]);
    const dirty = changed.length > 0;
    const saving = saveApp.isPending || saveRetention.isPending;

    const blocker = useBlocker({ shouldBlockFn: () => dirty && !leaving.current, enableBeforeUnload: () => dirty && !leaving.current, withResolver: true });

    const edit = (patch: Partial<SettingsDraft>) => setEdits((current) => ({ ...current, ...patch }));
    const touch = (row: Row) => setTouched((current) => new Set(current).add(row));
    const discard = () => {
        setEdits({});
        setTouched(new Set());
    };
    const shown = (row: Row) => (touched.has(row) && changed.includes(row) ? (errors[row] ?? null) : null);
    const changedFrom = (row: Row) => (changed.includes(row) ? valueText(row, baseline) : null);

    /** Writes the application rows to the device and the retention to the controller. */
    async function commit(rows: Row[], next: SettingsDraft, current: AppSettings): Promise<boolean> {
        try {
            if (rows.some((row) => APPLICATION_ROWS.includes(row))) await saveApp.mutateAsync(appSettingsFrom(current, next));
            if (rows.includes("retentionDays") && next.retentionDays !== null) await saveRetention.mutateAsync(next.retentionDays);
            return true;
        } catch (code) {
            notify.error("Settings not saved", { code: String(code), message: "Your edits are still here; save again to retry." });
            return false;
        }
    }

    function announce(rows: Row[], next: SettingsDraft, undo?: () => void) {
        const description = rows.length === 1 ? `${ROW_LABELS[rows[0]]} is now ${valueText(rows[0], next)}.` : `${plural(rows.length, "change")} saved.`;
        notify.success("Settings saved", { description, action: undo ? { label: "UNDO", onClick: undo } : undefined });
    }

    async function save() {
        if (!stored || invalid.length > 0 || saving) return;
        if (changed.includes("agentUrl")) {
            setSwitchTo(draft.agentUrl.trim());
            return;
        }
        const rows = changed;
        const next = draft;
        const previous = baseline;
        if (!(await commit(rows, next, stored))) return;
        discard();
        announce(rows, next, () => {
            void commit(rows, previous, appSettingsFrom(stored, next)).then((restored) => restored && notify.success("Settings restored"));
        });
    }

    async function confirmSwitch() {
        if (!stored || switchTo === null) return;
        const rows = changed.filter((row) => row !== "agentUrl");
        if (rows.length > 0) {
            if (!(await commit(rows, draft, stored))) {
                setSwitchTo(null);
                return;
            }
            announce(rows, draft);
        }
        leaving.current = true;
        setSwitchTo(null);
        if (!(await switchAgent(switchTo))) leaving.current = false;
    }

    if (!stored || !session) {
        return (
            <div className='flex flex-col gap-5'>
                <PageHeader title='SETTINGS' subtitle='// application & agent configuration' />
                <Skeleton className='h-80 w-full rounded-md' />
            </div>
        );
    }

    const refreshOptions = withCurrent(REFRESH_INTERVALS, baseline.refreshInterval).map((value) => ({ value: String(value), label: seconds(value) }));
    const scrollbackOptions = withCurrent(SCROLLBACKS, baseline.scrollback).map((value) => ({ value: String(value), label: value.toLocaleString("en-US") }));
    const retentionOptions = withCurrent(RETENTION_DAYS, baseline.retentionDays).map((value) => ({ value: String(value), label: days(value) }));
    const shellOptions = SHELLS.map((value) => ({ value, label: value }));
    const openElsewhere = [terminals > 0 && plural(terminals, "open terminal"), logStreams > 0 && plural(logStreams, "log stream")].filter(Boolean);

    return (
        <div className='flex flex-col gap-5'>
            <PageHeader title='SETTINGS' subtitle='// application & agent configuration' />

            <SettingsSection icon={Monitor} title='Application'>
                <SettingsRow
                    id='setting-theme'
                    label='Theme'
                    description='Follow the OS appearance, or force light or dark'
                    changedFrom={changedFrom("scheme")}
                >
                    <ChoiceSelect
                        id='setting-theme'
                        value={draft.scheme}
                        options={THEMES}
                        changed={changed.includes("scheme")}
                        onChange={(value) => edit({ scheme: value as ColorScheme })}
                    />
                </SettingsRow>
                <SettingsRow
                    id='setting-refresh'
                    label='Dashboard Refresh Interval'
                    description='How often the dashboard auto-refreshes cluster data'
                    changedFrom={changedFrom("refreshInterval")}
                >
                    <ChoiceSelect
                        id='setting-refresh'
                        value={String(draft.refreshInterval)}
                        options={refreshOptions}
                        changed={changed.includes("refreshInterval")}
                        onChange={(value) => edit({ refreshInterval: Number(value) })}
                    />
                </SettingsRow>
                <SettingsRow
                    id='setting-shell'
                    label='Default Terminal Shell'
                    description='Shell used when opening exec sessions'
                    changedFrom={changedFrom("shell")}
                    error={shown("shell")}
                >
                    <div className='flex items-center gap-2'>
                        {draft.shell === "custom" && (
                            <Input
                                aria-label='Custom shell path'
                                placeholder='/bin/ash'
                                value={draft.customShell}
                                aria-invalid={shown("shell") ? true : undefined}
                                aria-describedby={shown("shell") ? fieldHelperId("setting-shell") : undefined}
                                onChange={(event) => edit({ customShell: event.target.value })}
                                onBlur={() => touch("shell")}
                                className='h-8 w-44 font-mono text-xs'
                            />
                        )}
                        <ChoiceSelect
                            id='setting-shell'
                            value={draft.shell}
                            options={shellOptions}
                            changed={changed.includes("shell")}
                            onChange={(value) => edit({ shell: value as ShellChoice })}
                        />
                    </div>
                </SettingsRow>
                <SettingsRow
                    id='setting-font-size'
                    label='Terminal Font Size'
                    description='Font size for terminal sessions in pixels'
                    changedFrom={changedFrom("fontSize")}
                    error={shown("fontSize")}
                >
                    <FontSizeStepper
                        id='setting-font-size'
                        value={draft.fontSize}
                        invalid={Boolean(shown("fontSize"))}
                        onChange={(fontSize) => edit({ fontSize })}
                        onBlur={() => touch("fontSize")}
                    />
                </SettingsRow>
                <SettingsRow
                    id='setting-scrollback'
                    label='Terminal Scrollback Buffer'
                    description='Maximum number of lines kept in scrollback history'
                    changedFrom={changedFrom("scrollback")}
                >
                    <ChoiceSelect
                        id='setting-scrollback'
                        value={String(draft.scrollback)}
                        options={scrollbackOptions}
                        changed={changed.includes("scrollback")}
                        onChange={(value) => edit({ scrollback: Number(value) })}
                    />
                </SettingsRow>
            </SettingsSection>

            <SettingsSection icon={Server} title='Agent' badge={status.data ? `v${status.data.version}` : undefined}>
                <SettingsRow
                    id='setting-agent-url'
                    label='Agent URL'
                    description='Endpoint address of the connected agent'
                    changedFrom={changedFrom("agentUrl")}
                    error={shown("agentUrl")}
                >
                    <Input
                        id='setting-agent-url'
                        type='url'
                        spellCheck={false}
                        value={draft.agentUrl}
                        aria-invalid={shown("agentUrl") ? true : undefined}
                        aria-describedby={shown("agentUrl") ? fieldHelperId("setting-agent-url") : undefined}
                        onChange={(event) => edit({ agentUrl: event.target.value })}
                        onBlur={() => touch("agentUrl")}
                        className={cn("h-8 w-64 font-mono text-xs", changed.includes("agentUrl") && !shown("agentUrl") && "border-primary")}
                    />
                </SettingsRow>
                <SettingsRow id='setting-auth-provider' label='Auth Provider' description='How you sign in to this agent'>
                    <Input id='setting-auth-provider' readOnly value='Password' className={READ_ONLY} />
                </SettingsRow>
                <SettingsRow id='setting-log-level' label='Log Level' description='Minimum log severity reported by the agent'>
                    <LabelTooltip label='Set with RUST_LOG on the controller'>
                        <Input
                            id='setting-log-level'
                            readOnly
                            value={status.data?.logLevel ?? "—"}
                            title={status.data?.logLevel}
                            className={cn(READ_ONLY, "truncate")}
                        />
                    </LabelTooltip>
                </SettingsRow>
                <SettingsRow
                    id='setting-retention'
                    label='Metrics Retention'
                    description='How long node metrics are stored before pruning'
                    changedFrom={changedFrom("retentionDays")}
                >
                    {draft.retentionDays === null ? (
                        <Skeleton className='h-8 w-24 rounded-md' />
                    ) : (
                        <ChoiceSelect
                            id='setting-retention'
                            value={String(draft.retentionDays)}
                            options={retentionOptions}
                            changed={changed.includes("retentionDays")}
                            onChange={(value) => edit({ retentionDays: Number(value) })}
                        />
                    )}
                </SettingsRow>
                <SettingsRow id='setting-signed-in' label='Signed in as' description='The account this device uses on the agent'>
                    <Input id='setting-signed-in' readOnly value={`${session.displayName} · ${session.email}`} className={cn(READ_ONLY, "w-64 truncate")} />
                </SettingsRow>
                <SettingsRow id='setting-uptime' label='Uptime' description='Time since the controller started'>
                    <Uptime id='setting-uptime' startedAt={status.data?.startedAt} />
                </SettingsRow>
            </SettingsSection>

            <SettingsSection icon={Plug} title='Integrations'>
                <SettingsRow
                    id='setting-integrations'
                    label='Integrations hub'
                    description='GitHub, Slack, Discord, webhooks and Traefik — connection status and per-integration settings'
                >
                    <ActionButton
                        id='setting-integrations'
                        variant='outline'
                        icon={ExternalLink}
                        restricted={{ title: "Integrations aren't available yet", reason: "The integrations hub arrives in a later build." }}
                    >
                        OPEN
                    </ActionButton>
                </SettingsRow>
            </SettingsSection>

            {dirty && <PendingChangesBar changes={changed.length} invalid={invalid} saving={saving} onSave={() => void save()} onDiscard={discard} />}

            <ConfirmDialog
                open={switchTo !== null}
                onOpenChange={(open) => !open && setSwitchTo(null)}
                tone='neutral'
                icon={ArrowLeftRight}
                title={`Switch to ${switchTo ?? ""}?`}
                description={`You'll be signed out of ${session.label ?? session.origin} and asked to sign in again on the new agent. Settings stored on this device are kept.`}
                impact={openElsewhere.length > 0 ? `${openElsewhere.join(" and ")} will close.` : undefined}
                confirmLabel='SWITCH & SIGN IN'
                confirmIcon={LogIn}
                mutates={false}
                pending={saving}
                onConfirm={() => void confirmSwitch()}
            />
            <ConfirmDialog
                open={blocker.status === "blocked"}
                onOpenChange={(open) => !open && blocker.reset?.()}
                tone='neutral'
                icon={LogOut}
                title='Leave with unsaved changes?'
                description={`${plural(changed.length, "unsaved change")} on this page will be lost.`}
                confirmLabel='LEAVE'
                confirmIcon={LogOut}
                mutates={false}
                onConfirm={() => blocker.proceed?.()}
            />
        </div>
    );
}
