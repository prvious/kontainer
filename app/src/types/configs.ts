export interface ConfigItem {
    id: string;
    name: string;
    labels: Record<string, string>;
    dataSize: number;
    createdAt: string;
    updatedAt: string;
}

export interface ConfigDetail {
    id: string;
    name: string;
    labels: Record<string, string>;
    data: string;
    createdAt: string;
    updatedAt: string;
}
