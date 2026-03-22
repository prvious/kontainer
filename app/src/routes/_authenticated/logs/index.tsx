import { useState } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { toast } from "sonner";
import { Plus, ScrollText } from "lucide-react";
import { PageHeader } from "@/components/page-header";
import { LogStreamTabs } from "@/components/logs/log-stream-tabs";
import type { LogStream } from "@/components/logs/log-stream-tabs";
import { LogPanel } from "@/components/logs/log-panel";
import { StreamPickerDialog } from "@/components/logs/stream-picker-dialog";
import { Button } from "@/components/ui/button";

const TAB_COLORS = ["bg-accent-green", "bg-accent-cyan", "bg-accent-orange", "bg-accent-purple", "bg-accent-red"];
const MAX_STREAMS = 10;

function LogsPage() {
    const [streams, setStreams] = useState<LogStream[]>([]);
    const [activeStreamId, setActiveStreamId] = useState<string | null>(null);
    const [pickerOpen, setPickerOpen] = useState(false);

    function openPicker() {
        if (streams.length >= MAX_STREAMS) {
            toast.warning(`Maximum ${MAX_STREAMS} streams allowed. Close a tab first.`);
            return;
        }
        setPickerOpen(true);
    }

    function handleAddStream(service: { id: string; name: string }) {
        if (streams.length >= MAX_STREAMS) return;
        const id = crypto.randomUUID();
        setStreams((prev) => [
            ...prev,
            {
                id,
                serviceId: service.id,
                serviceName: service.name,
                color: TAB_COLORS[prev.length % TAB_COLORS.length],
            },
        ]);
        setActiveStreamId(id);
        setPickerOpen(false);
    }

    function handleCloseStream(closeId: string) {
        let nextStreams: LogStream[] = [];
        setStreams((prev) => {
            nextStreams = prev.filter((s) => s.id !== closeId);
            return nextStreams;
        });
        setActiveStreamId((currentActive) => {
            if (currentActive !== closeId) return currentActive;
            return nextStreams.length > 0 ? nextStreams[nextStreams.length - 1].id : null;
        });
    }

    function handleSelectTab(id: string) {
        setActiveStreamId(id);
    }

    const hasStreams = streams.length > 0;

    return (
        <div className='flex h-full flex-col gap-4 p-6'>
            <PageHeader title='LOGS' subtitle='// per-service log streams'>
                <button
                    onClick={openPicker}
                    className='flex items-center gap-1.5 rounded-md border border-accent-green bg-accent-green-20 px-3 py-1.5 font-mono text-[11px] font-semibold text-accent-green'
                >
                    <Plus className='size-3.5' />
                    New Stream
                </button>
            </PageHeader>

            {hasStreams ? (
                <>
                    <LogStreamTabs
                        streams={streams}
                        activeStreamId={activeStreamId}
                        onSelect={handleSelectTab}
                        onClose={handleCloseStream}
                        onAdd={openPicker}
                    />
                    <div className='flex flex-1 flex-col overflow-hidden rounded-b-lg border-x border-b border-border'>
                        {streams.map((stream) => (
                            <LogPanel key={stream.id} serviceId={stream.serviceId} visible={stream.id === activeStreamId} logFilter='' />
                        ))}
                    </div>
                </>
            ) : (
                <div className='flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground'>
                    <ScrollText className='size-8' />
                    <p className='font-mono text-sm'>No log streams active</p>
                    <p className='font-mono text-xs'>Click &quot;New Stream&quot; to start watching service logs</p>
                    <Button variant='outline' size='sm' onClick={openPicker} className='mt-2 gap-1.5'>
                        <Plus className='size-3.5' />
                        Add Stream
                    </Button>
                </div>
            )}

            <StreamPickerDialog open={pickerOpen} onOpenChange={setPickerOpen} onSelect={handleAddStream} excludeServiceIds={streams.map((s) => s.serviceId)} />
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/logs/")({
    component: LogsPage,
});
