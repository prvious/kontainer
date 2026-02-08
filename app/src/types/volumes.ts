export interface VolumeItem {
    name: string;
    driver: string;
    scope: string;
    mountpoint: string;
    labels: Record<string, string>;
    options: Record<string, string>;
    createdAt: string;
    refCount: number;
    sizeBytes: number;
    stack: string;
}
