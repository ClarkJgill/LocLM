export type FitLabel = "runsWell" | "mightBeSlow" | "notRecommended";

export type DownloadPhase =
  | "idle"
  | "downloading"
  | "paused"
  | "verifying"
  | "completed"
  | "error";

export interface CatalogModel {
  id: string;
  name: string;
  family: string;
  paramsB: number;
  quantization: string;
  sizeBytes: number;
  hfRepo: string;
  hfFilename: string;
  downloadUrl: string;
  sha256: string;
  description: string;
}

export interface ModelLibraryEntry {
  model: CatalogModel;
  fit: FitLabel;
  fitRatio: number;
  fitDetail: string;
  localPath: string | null;
  downloaded: boolean;
  partialBytes: number;
}

export interface DownloadProgress {
  modelId: string;
  phase: DownloadPhase;
  downloadedBytes: number;
  totalBytes: number;
  bytesPerSec: number;
  percent: number;
  message: string;
}
