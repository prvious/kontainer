import { createFileRoute } from "@tanstack/react-router";
import { Container, Github, ShieldCheck, TriangleAlert } from "lucide-react";
import { PageHeader } from "@/components/page-header";
import { StatusBadge } from "@/components/kit/badges";
import { Breadcrumbs } from "@/components/kit/breadcrumbs";
import { InlineAlert } from "@/components/kit/inline-alert";
import { DeferredSection } from "@/components/integrations/deferred-section";

export const Route = createFileRoute("/_authenticated/settings/integrations/github")({
    component: GitHubIntegration,
});

/** The GitHub integration page `kx4iP` in its deferred state (G23): section headers over skeletons. */
function GitHubIntegration() {
    return (
        <div className='flex flex-col gap-5'>
            <Breadcrumbs trail={[{ label: "Settings", to: "/settings" }, { label: "Integrations", to: "/settings/integrations" }, { label: "GitHub" }]} />
            <PageHeader title='GITHUB INTEGRATION' subtitle='// repository access & deployment policies'>
                <StatusBadge tone='neutral'>NOT CONNECTED</StatusBadge>
            </PageHeader>
            <InlineAlert tone='info' title='GitHub connects when deployments arrive.' />
            <div className='grid grid-cols-1 items-start gap-5 lg:grid-cols-2'>
                <div className='flex flex-col gap-5'>
                    <DeferredSection icon={Github} title='OAuth Connection' rows={3} />
                    <DeferredSection icon={Container} title='Container Registry (GHCR)' rows={3} />
                </div>
                <div className='flex flex-col gap-5'>
                    <DeferredSection icon={ShieldCheck} title='OIDC Deploy Policies' rows={4} />
                    <DeferredSection icon={TriangleAlert} title='Danger Zone' rows={2} danger />
                </div>
            </div>
        </div>
    );
}
