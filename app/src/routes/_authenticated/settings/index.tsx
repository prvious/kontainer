import { createFileRoute } from "@tanstack/react-router";
import { GeneralSection } from "@/components/settings/general-tab";
import { AppearanceSection } from "@/components/settings/appearance-tab";
import { TerminalSection } from "@/components/settings/terminal-tab";
import { ConnectionSection } from "@/components/settings/connection-tab";

function SettingsPage() {
    return (
        <div className='mx-auto max-w-2xl'>
            <div className='mb-8'>
                <h1 className='text-xl font-semibold tracking-tight'>Settings</h1>
                <p className='mt-1 text-sm text-muted-foreground'>Application and cluster configuration</p>
            </div>
            <div className='space-y-8'>
                <GeneralSection />
                <AppearanceSection />
                <TerminalSection />
                <ConnectionSection />
            </div>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/settings/")({
    component: SettingsPage,
});
