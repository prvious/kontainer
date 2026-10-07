import { Toaster } from "@/components/ui/sonner";
import { useAppearance } from "@/hooks/use-appearance";

/** The one toaster: bottom-right, 5 s, following the resolved light/dark scheme. */
export function AppToaster() {
    const { resolvedScheme } = useAppearance();
    return (
        <Toaster
            theme={resolvedScheme}
            position='bottom-right'
            duration={5000}
            visibleToasts={5}
            toastOptions={{
                classNames: {
                    toast: "font-mono text-xs",
                    description: "font-mono text-2xs !text-muted-foreground whitespace-pre-line",
                    actionButton: "!bg-transparent !border !border-border !text-secondary-foreground !font-mono !text-3xs !font-semibold",
                },
            }}
        />
    );
}
