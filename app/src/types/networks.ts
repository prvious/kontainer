export interface NetworkServiceRef {
    id: string;
    name: string;
}

export interface NetworkItem {
    id: string;
    name: string;
    driver: string;
    scope: string;
    internal: boolean;
    attachable: boolean;
    ingress: boolean;
    services: NetworkServiceRef[];
    createdAt: string;
}
