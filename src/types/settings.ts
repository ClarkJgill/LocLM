export interface InferenceSettings {
  contextLength: number;
  temperature: number;
  gpuLayers: number;
  threadCount: number;
  maxTokens: number;
  userCustomized: boolean;
}
