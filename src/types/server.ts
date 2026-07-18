export type ServerPhase =
  | "stopped"
  | "starting"
  | "ready"
  | "unhealthy"
  | "error";

export interface ServerStatus {
  phase: ServerPhase;
  port: number | null;
  pid: number | null;
  baseUrl: string | null;
  binaryPath: string | null;
  modelPath: string | null;
  message: string;
}
