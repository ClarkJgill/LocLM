/** Stream chat completions from the local llama.cpp OpenAI-compatible API. */

export interface ChatApiMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

export interface StreamChatOptions {
  baseUrl: string;
  messages: ChatApiMessage[];
  temperature?: number;
  maxTokens?: number;
  signal?: AbortSignal;
  onToken: (token: string) => void;
}

export async function streamChatCompletion(
  opts: StreamChatOptions,
): Promise<void> {
  const response = await fetch(`${opts.baseUrl}/v1/chat/completions`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      messages: opts.messages,
      temperature: opts.temperature ?? 0.7,
      max_tokens: opts.maxTokens ?? 1024,
      stream: true,
    }),
    signal: opts.signal,
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(`Chat request failed (${response.status}): ${text}`);
  }

  if (!response.body) {
    throw new Error("No response body from llama-server");
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });

    const lines = buffer.split("\n");
    buffer = lines.pop() ?? "";

    for (const raw of lines) {
      const line = raw.trim();
      if (!line || !line.startsWith("data:")) continue;
      const data = line.slice(5).trim();
      if (data === "[DONE]") return;
      try {
        const json = JSON.parse(data) as {
          choices?: Array<{ delta?: { content?: string | null } }>;
        };
        const token = json.choices?.[0]?.delta?.content;
        if (token) opts.onToken(token);
      } catch {
        // skip malformed SSE chunks
      }
    }
  }
}
