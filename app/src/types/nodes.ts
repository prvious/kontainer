export interface NodeItem {
    id: string;
    hostname: string;
    role: string;
    status: string;
    availability: string;
    engineVersion: string;
    os: string;
    architecture: string;
    ip: string;
    leader: boolean;
    updatedAt: string;
}
