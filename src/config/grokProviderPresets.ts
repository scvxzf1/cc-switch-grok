/**
 * Grok Build provider presets configuration.
 * Grok CLI uses ~/.grok/config.toml model tables + models.default.
 */
import type { ProviderCategory } from "../types";
import type { PresetTheme, TemplateValueConfig } from "./claudeProviderPresets";

export type GrokApiBackend = "chat_completions" | "responses" | "messages";

export interface GrokModelMapping {
  id: string;
  upstreamModel: string;
  displayName?: string;
  contextWindow?: number;
  isDefault?: boolean;
}

export interface GrokChatReasoning {
  supportsThinking?: boolean;
  supportsEffort?: boolean;
  thinkingParam?: "thinking" | "enable_thinking" | "reasoning_split";
  effortParam?: "reasoning_effort" | "reasoning.effort" | "none";
  effortValueMode?: "openai" | "openrouter";
}

export interface GrokProviderSettingsConfig {
  kind: "official" | "custom";
  model: string;
  base_url?: string;
  api_key?: string;
  env_key?: string;
  api_backend?: GrokApiBackend;
  name?: string;
  description?: string;
  context_window?: number;
  useLocalRoute?: boolean;
  modelMappings?: GrokModelMapping[];
  grokChatReasoning?: GrokChatReasoning;
  defaultReasoningEffort?: "low" | "medium" | "high" | "xhigh" | "max";
  [key: string]: unknown;
}

export interface GrokProviderPreset {
  name: string;
  nameKey?: string;
  websiteUrl: string;
  apiKeyUrl?: string;
  settingsConfig: GrokProviderSettingsConfig;
  isOfficial?: boolean;
  isPartner?: boolean;
  category?: ProviderCategory;
  templateValues?: Record<string, TemplateValueConfig>;
  theme?: PresetTheme;
  icon?: string;
  iconColor?: string;
  isCustomTemplate?: boolean;
}

export const grokProviderPresets: GrokProviderPreset[] = [
  {
    name: "xAI Official",
    websiteUrl: "https://grok.com",
    isOfficial: true,
    category: "official",
    icon: "xai",
    settingsConfig: {
      kind: "official",
      model: "grok-build",
      name: "Grok Build",
      useLocalRoute: false,
      modelMappings: [
        {
          id: "grok-build",
          upstreamModel: "grok-build",
          displayName: "Grok Build",
          isDefault: true,
        },
      ],
    },
  },
  {
    name: "Custom OpenAI-Compatible",
    websiteUrl: "",
    isCustomTemplate: true,
    category: "custom",
    icon: "openai",
    settingsConfig: {
      kind: "custom",
      model: "",
      name: "Custom Proxy",
      base_url: "",
      api_key: "",
      api_backend: "chat_completions",
      useLocalRoute: false,
      modelMappings: [
        {
          id: "custom",
          upstreamModel: "",
          displayName: "Custom",
          isDefault: true,
        },
      ],
      grokChatReasoning: {
        supportsThinking: false,
        supportsEffort: false,
      },
    },
  },
];
