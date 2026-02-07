export interface ServiceItem {
    id: string;
    name: string;
    image: string;
    stack: string;
    replicasRunning: number;
    replicasDesired: number;
    updatedAt: string;
}
