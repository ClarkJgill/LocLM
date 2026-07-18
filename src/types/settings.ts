export interface InferenceSettings {
  contextLength: number;
  temperature: number;
  gpuLayers: number;
  threadCount: number;
  maxTokens: number;
  userCustomized: boolean;
  lastModelId: string | null;
  onboardingComplete: boolean;
  smartscreenAcked: boolean;
}
