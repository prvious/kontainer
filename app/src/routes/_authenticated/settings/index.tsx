import { createFileRoute } from "@tanstack/react-router";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { GeneralTab } from "@/components/settings/general-tab";
import { AppearanceTab } from "@/components/settings/appearance-tab";
import { TerminalTab } from "@/components/settings/terminal-tab";
import { ConnectionTab } from "@/components/settings/connection-tab";

function SettingsPage() {
    return (
        <div className='p-6'>
            <div className='mb-6'>
                <h1 className='text-xl font-semibold tracking-tight'>Settings</h1>
                <p className='mt-1 text-sm text-muted-foreground'>Application and cluster configuration</p>
            </div>

            <Tabs defaultValue='general'>
                <TabsList>
                    <TabsTrigger value='general'>General</TabsTrigger>
                    <TabsTrigger value='appearance'>Appearance</TabsTrigger>
                    <TabsTrigger value='terminal'>Terminal</TabsTrigger>
                    <TabsTrigger value='connection'>Connection</TabsTrigger>
                </TabsList>

                <TabsContent value='general'>
                    <GeneralTab />
                </TabsContent>
                <TabsContent value='appearance'>
                    <AppearanceTab />
                </TabsContent>
                <TabsContent value='terminal'>
                    <TerminalTab />
                </TabsContent>
                <TabsContent value='connection'>
                    <ConnectionTab />
                </TabsContent>
            </Tabs>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/settings/")({
    component: SettingsPage,
});
