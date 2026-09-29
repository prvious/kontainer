import { createFileRoute } from "@tanstack/react-router";
import { Github, Hash, MessageCircle, Webhook } from "lucide-react";
import { PageHeader } from "@/components/page-header";
import { Breadcrumbs } from "@/components/kit/breadcrumbs";
import { IntegrationCard } from "@/components/integrations/integration-card";
import { TraefikPanel } from "@/components/integrations/traefik-panel";

export const Route = createFileRoute("/_authenticated/settings/integrations/")({
    component: IntegrationsHub,
});

/** The Integrations hub `r9tns`, deferred (ADR 0010): four cards and the Traefik panel. */
function IntegrationsHub() {
    return (
        <div className='flex flex-col gap-5'>
            <Breadcrumbs trail={[{ label: "Settings", to: "/settings" }, { label: "Integrations" }]} />
            <PageHeader title='INTEGRATIONS' subtitle='// manage external service connections' />
            <div className='grid grid-cols-2 gap-4 xl:grid-cols-4'>
                <IntegrationCard icon={Github} name='GitHub' to='/settings/integrations/github' />
                <IntegrationCard icon={Hash} name='Slack' />
                <IntegrationCard icon={MessageCircle} name='Discord' />
                <IntegrationCard icon={Webhook} name='Webhooks' />
            </div>
            <TraefikPanel />
        </div>
    );
}
