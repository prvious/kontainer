export interface Column {
    label: string;
    width?: string | number;
    align?: "left" | "right" | "center";
}

interface ColumnHeadersProps {
    columns: Column[];
}

export function ColumnHeaders({ columns }: ColumnHeadersProps) {
    return (
        <div className='flex items-center border-b border-border bg-bg-elevated px-4 py-2'>
            {columns.map((col) => (
                <span
                    key={col.label || "actions"}
                    className={`font-mono text-[9px] font-bold tracking-[0.5px] text-muted-foreground uppercase ${
                        col.align === "right" ? "text-right" : col.align === "center" ? "text-center" : ""
                    }`}
                    style={{ width: typeof col.width === "number" ? col.width : (col.width ?? "auto"), flex: col.width ? undefined : 1 }}
                >
                    {col.label}
                </span>
            ))}
        </div>
    );
}
