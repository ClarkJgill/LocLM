import { invoke } from "@tauri-apps/api/core";
import type { Conversation } from "../types/chat";
import type { HardwareInfo } from "../types/hardware";
import type {
  DownloadProgress,
  ModelLibraryEntry,
} from "../types/models";
import type { ServerStatus } from "../types/server";
import type { InferenceSettings } from "../types/settings";

export function getHardwareInfo(): Promise<HardwareInfo> {
  return invoke<HardwareInfo>("get_hardware_info");
}

export function getServerStatus(): Promise<ServerStatus> {
  return invoke<ServerStatus>("get_server_status");
}

export function startInferenceServer(opts?: {
  modelPath?: string;
  gpuLayers?: number;
  contextLength?: number;
  threadCount?: number;
}): Promise<ServerStatus> {
  return invoke<ServerStatus>("start_inference_server", {
    modelPath: opts?.modelPath ?? null,
    gpuLayers: opts?.gpuLayers ?? null,
    contextLength: opts?.contextLength ?? null,
    threadCount: opts?.threadCount ?? null,
  });
}

export function stopInferenceServer(): Promise<ServerStatus> {
  return invoke<ServerStatus>("stop_inference_server");
}

export function checkInferenceHealth(): Promise<ServerStatus> {
  return invoke<ServerStatus>("check_inference_health");
}

export function resolveLlamaBinary(): Promise<string> {
  return invoke<string>("resolve_llama_binary");
}

export function listModelLibrary(): Promise<ModelLibraryEntry[]> {
  return invoke<ModelLibraryEntry[]>("list_model_library");
}

export function getModelsDir(): Promise<string> {
  return invoke<string>("get_models_dir");
}

export function startModelDownload(modelId: string): Promise<void> {
  return invoke("start_model_download", { modelId });
}

export function pauseModelDownload(modelId: string): Promise<void> {
  return invoke("pause_model_download", { modelId });
}

export function resumeModelDownload(modelId: string): Promise<void> {
  return invoke("resume_model_download", { modelId });
}

export function cancelModelDownload(modelId: string): Promise<void> {
  return invoke("cancel_model_download", { modelId });
}

export function getDownloadProgress(): Promise<DownloadProgress | null> {
  return invoke<DownloadProgress | null>("get_download_progress");
}

export function deleteLocalModel(modelId: string): Promise<void> {
  return invoke("delete_local_model", { modelId });
}

export function listConversations(): Promise<Conversation[]> {
  return invoke<Conversation[]>("list_conversations");
}

export function loadConversation(id: string): Promise<Conversation> {
  return invoke<Conversation>("load_conversation", { id });
}

export function saveConversation(
  conversation: Conversation,
): Promise<Conversation> {
  return invoke<Conversation>("save_conversation", { conversation });
}

export function deleteConversation(id: string): Promise<void> {
  return invoke("delete_conversation", { id });
}

export function newConversation(modelId?: string | null): Promise<Conversation> {
  return invoke<Conversation>("new_conversation", {
    modelId: modelId ?? null,
  });
}

export function getInferenceSettings(): Promise<InferenceSettings> {
  return invoke<InferenceSettings>("get_inference_settings");
}

export function saveInferenceSettings(
  settings: InferenceSettings,
): Promise<InferenceSettings> {
  return invoke<InferenceSettings>("save_inference_settings", { settings });
}

export function resetInferenceSettings(): Promise<InferenceSettings> {
  return invoke<InferenceSettings>("reset_inference_settings");
}
