export interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  createdAt: number;
}

export interface Conversation {
  id: string;
  title: string;
  modelId: string | null;
  systemPrompt: string;
  messages: ChatMessage[];
  updatedAt: number;
}
