export interface VMProgress {
    stage: string;
    step: number;
    totalSteps: number;
    message: string;
    percent: number;
    timestamp?: number;
    tunnelUrl?: string;
    elapsedSeconds?: number;
}
