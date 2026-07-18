export type GpuVendor =
  | "nvidia"
  | "amd"
  | "apple"
  | "intel"
  | "other"
  | "none";

export type InferenceBackend = "cuda" | "vulkan" | "metal" | "cpu";

export interface GpuInfo {
  name: string;
  vendor: GpuVendor;
  vramMb: number | null;
}

export interface RecommendedSettings {
  maxModelParamsB: number;
  quantization: string;
  contextLength: number;
  gpuLayers: number;
  threadCount: number;
  summary: string;
}

export interface HardwareInfo {
  cpuBrand: string;
  cpuCoresLogical: number;
  cpuCoresPhysical: number;
  totalMemoryMb: number;
  availableMemoryMb: number;
  gpus: GpuInfo[];
  primaryBackend: InferenceBackend;
  recommended: RecommendedSettings;
}
