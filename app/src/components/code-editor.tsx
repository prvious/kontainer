import CodeMirror from "@uiw/react-codemirror";
import { json } from "@codemirror/lang-json";
import { yaml } from "@codemirror/lang-yaml";
import { oneDark } from "@codemirror/theme-one-dark";
import { useAppearance } from "@/hooks/use-appearance";

interface CodeEditorProps {
    value: string;
    onChange?: (value: string) => void;
    readOnly?: boolean;
    language?: "json" | "yaml" | "plaintext";
    minHeight?: string;
    maxHeight?: string;
    height?: string;
    className?: string;
    fill?: boolean;
}

const languageExtensions = {
    json: [json()],
    yaml: [yaml()],
    plaintext: [],
};

export function CodeEditor({
    value,
    onChange,
    readOnly = false,
    language = "plaintext",
    minHeight = "200px",
    maxHeight,
    height,
    className,
    fill,
}: CodeEditorProps) {
    const { resolvedScheme } = useAppearance();

    const extensions = [...languageExtensions[language]];

    return (
        <div className={`overflow-hidden rounded-md border border-border ${className ?? ""}`.trim()}>
            <CodeMirror
                className={fill ? '!h-full [&_.cm-editor]:!h-full [&_.cm-scroller]:!overflow-auto' : undefined}
                value={value}
                onChange={onChange}
                readOnly={readOnly}
                extensions={extensions}
                theme={resolvedScheme === "dark" ? oneDark : "light"}
                minHeight={minHeight}
                maxHeight={maxHeight}
                height={fill ? "100%" : height}
                basicSetup={{
                    lineNumbers: true,
                    foldGutter: true,
                    highlightActiveLine: !readOnly,
                }}
            />
        </div>
    );
}
