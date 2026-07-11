import { useCallback, useMemo, useState } from "react";
import type { AppId } from "@/lib/api";
import { useProvidersQuery } from "@/lib/query/queries";
import type {
  GrokApiBackend,
  GrokChatReasoning,
  GrokModelMapping,
  GrokProviderSettingsConfig,
} from "@/config/grokProviderPresets";

export type GrokReasoningEffort =
  | "low"
  | "medium"
  | "high"
  | "xhigh"
  | "max";

export type GrokValidationResult =
  | { ok: true }
  | { ok: false; errorKey: string };

interface UseGrokFormStateParams {
  initialData?: {
    settingsConfig?: Record<string, unknown>;
  };
  appId: AppId;
  providerId?: string;
  onSettingsConfigChange: (config: string) => void;
  getSettingsConfig: () => string;
}

const GROK_DEFAULT_CONFIG_OBJ: GrokProviderSettingsConfig = {
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
};

export const GROK_DEFAULT_CONFIG = JSON.stringify(
  GROK_DEFAULT_CONFIG_OBJ,
  null,
  2,
);

export const GROK_API_BACKENDS: Array<{
  value: GrokApiBackend;
  labelKey: string;
}> = [
  {
    value: "chat_completions",
    labelKey: "grok.form.apiBackendChatCompletions",
  },
  { value: "responses", labelKey: "grok.form.apiBackendResponses" },
  { value: "messages", labelKey: "grok.form.apiBackendMessages" },
];

export const GROK_REASONING_EFFORTS: GrokReasoningEffort[] = [
  "low",
  "medium",
  "high",
  "xhigh",
  "max",
];

function parseGrokField<T>(
  initialData: UseGrokFormStateParams["initialData"],
  field: string,
  fallback: T,
): T {
  try {
    if (initialData?.settingsConfig) {
      const value = initialData.settingsConfig[field];
      return (value as T) ?? fallback;
    }
    return (
      ((GROK_DEFAULT_CONFIG_OBJ as Record<string, unknown>)[field] as T) ??
      fallback
    );
  } catch {
    return fallback;
  }
}

function normalizeMappings(
  mappings: GrokModelMapping[] | undefined,
): GrokModelMapping[] {
  if (!Array.isArray(mappings)) return [];
  return mappings.map((item) => ({
    id: item?.id ?? "",
    upstreamModel: item?.upstreamModel ?? "",
    displayName: item?.displayName,
    contextWindow:
      typeof item?.contextWindow === "number" ? item.contextWindow : undefined,
    isDefault: item?.isDefault === true,
  }));
}

export function applyGrokReasoningToggle(
  current: GrokChatReasoning | undefined,
  field: "thinking" | "effort",
  checked: boolean,
): GrokChatReasoning {
  const base: GrokChatReasoning = {
    supportsThinking: current?.supportsThinking === true,
    supportsEffort: current?.supportsEffort === true,
    thinkingParam: current?.thinkingParam,
    effortParam: current?.effortParam,
    effortValueMode: current?.effortValueMode,
  };

  if (field === "thinking") {
    return {
      ...base,
      supportsThinking: checked,
      supportsEffort: checked ? base.supportsEffort : false,
      effortParam: checked
        ? (base.effortParam ??
          (base.supportsEffort ? "reasoning_effort" : "none"))
        : "none",
    };
  }

  return {
    ...base,
    supportsThinking: checked ? true : base.supportsThinking,
    supportsEffort: checked,
    effortParam: checked
      ? (base.effortParam && base.effortParam !== "none"
          ? base.effortParam
          : "reasoning_effort")
      : "none",
  };
}

function isHttpUrl(raw: string): boolean {
  try {
    const url = new URL(raw);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
}

export function validateGrokSettingsConfig(
  config: Partial<GrokProviderSettingsConfig> | Record<string, unknown>,
): GrokValidationResult {
  const kind = config.kind === "official" ? "official" : "custom";
  const useLocalRoute = config.useLocalRoute === true;
  const modelMappings = Array.isArray(config.modelMappings)
    ? (config.modelMappings as GrokModelMapping[])
    : [];

  if (kind === "official" && useLocalRoute) {
    return { ok: false, errorKey: "grok.form.localRouteOfficialDisabled" };
  }

  if (modelMappings.length < 1) {
    return { ok: false, errorKey: "grok.form.modelMappingsRequired" };
  }

  if (kind === "custom") {
    const baseUrl =
      typeof config.base_url === "string" ? config.base_url.trim() : "";
    if (!baseUrl) {
      return { ok: false, errorKey: "grok.form.baseUrlRequired" };
    }
    // 本地路由允许本地 proxy URL；直连必须是合法 http(s)
    if (!useLocalRoute && !isHttpUrl(baseUrl)) {
      return { ok: false, errorKey: "grok.form.baseUrlInvalid" };
    }
    if (useLocalRoute && !isHttpUrl(baseUrl) && !baseUrl.startsWith("/")) {
      // 本地路由也接受绝对 http(s)；其他非路径形式仍拒绝
      return { ok: false, errorKey: "grok.form.baseUrlInvalid" };
    }
  }

  return { ok: true };
}

export interface GrokFormState {
  grokProviderKey: string;
  setGrokProviderKey: (key: string) => void;
  grokBaseUrl: string;
  grokApiKey: string;
  grokApiBackend: GrokApiBackend;
  grokModel: string;
  grokUseLocalRoute: boolean;
  grokChatReasoning: GrokChatReasoning;
  grokDefaultReasoningEffort: GrokReasoningEffort;
  grokModelMappings: GrokModelMapping[];
  existingGrokKeys: string[];
  handleGrokBaseUrlChange: (baseUrl: string) => void;
  handleGrokApiKeyChange: (apiKey: string) => void;
  handleGrokApiBackendChange: (backend: GrokApiBackend) => void;
  handleGrokModelChange: (model: string) => void;
  handleGrokUseLocalRouteChange: (enabled: boolean) => void;
  handleGrokChatReasoningChange: (value: GrokChatReasoning) => void;
  handleGrokDefaultReasoningEffortChange: (effort: GrokReasoningEffort) => void;
  handleGrokModelMappingsChange: (mappings: GrokModelMapping[]) => void;
  resetGrokState: (config?: Partial<GrokProviderSettingsConfig>) => void;
  buildValidatedConfig: () =>
    | { ok: true; config: GrokProviderSettingsConfig }
    | { ok: false; errorKey: string };
}

export function useGrokFormState({
  initialData,
  appId,
  providerId,
  onSettingsConfigChange,
  getSettingsConfig,
}: UseGrokFormStateParams): GrokFormState {
  const { data: grokProvidersData } = useProvidersQuery("grok");
  const existingGrokKeys = useMemo(() => {
    if (!grokProvidersData?.providers) return [];
    return Object.keys(grokProvidersData.providers).filter(
      (k) => k !== providerId,
    );
  }, [grokProvidersData?.providers, providerId]);

  const [grokProviderKey, setGrokProviderKey] = useState<string>(() => {
    if (appId !== "grok") return "";
    return providerId || "";
  });

  const [grokBaseUrl, setGrokBaseUrl] = useState<string>(() => {
    if (appId !== "grok") return "";
    return parseGrokField(initialData, "base_url", "");
  });

  const [grokApiKey, setGrokApiKey] = useState<string>(() => {
    if (appId !== "grok") return "";
    return parseGrokField(initialData, "api_key", "");
  });

  const [grokApiBackend, setGrokApiBackend] = useState<GrokApiBackend>(() => {
    if (appId !== "grok") return "chat_completions";
    return (
      parseGrokField<GrokApiBackend | "">(
        initialData,
        "api_backend",
        "chat_completions",
      ) || "chat_completions"
    );
  });

  const [grokModel, setGrokModel] = useState<string>(() => {
    if (appId !== "grok") return "";
    return parseGrokField(initialData, "model", "grok-build");
  });

  const [grokUseLocalRoute, setGrokUseLocalRoute] = useState<boolean>(() => {
    if (appId !== "grok") return false;
    const kind = parseGrokField(initialData, "kind", "official");
    if (kind === "official") return false;
    return Boolean(parseGrokField(initialData, "useLocalRoute", false));
  });

  const [grokChatReasoning, setGrokChatReasoning] = useState<GrokChatReasoning>(
    () => {
      if (appId !== "grok") return {};
      return (
        parseGrokField<GrokChatReasoning | undefined>(
          initialData,
          "grokChatReasoning",
          {},
        ) ?? {}
      );
    },
  );

  const [grokDefaultReasoningEffort, setGrokDefaultReasoningEffort] =
    useState<GrokReasoningEffort>(() => {
      if (appId !== "grok") return "medium";
      const value = parseGrokField<GrokReasoningEffort | "">(
        initialData,
        "defaultReasoningEffort",
        "medium",
      );
      return value || "medium";
    });

  const [grokModelMappings, setGrokModelMappings] = useState<
    GrokModelMapping[]
  >(() => {
    if (appId !== "grok") return [];
    return normalizeMappings(
      parseGrokField<GrokModelMapping[] | undefined>(
        initialData,
        "modelMappings",
        GROK_DEFAULT_CONFIG_OBJ.modelMappings,
      ),
    );
  });

  const updateGrokConfig = useCallback(
    (updater: (config: Record<string, unknown>) => void) => {
      try {
        const config = JSON.parse(getSettingsConfig() || GROK_DEFAULT_CONFIG);
        updater(config);
        // 官方永远禁止本地路由
        if (config.kind === "official") {
          config.useLocalRoute = false;
        }
        onSettingsConfigChange(JSON.stringify(config, null, 2));
      } catch {
        // ignore
      }
    },
    [getSettingsConfig, onSettingsConfigChange],
  );

  const handleGrokBaseUrlChange = useCallback(
    (baseUrl: string) => {
      setGrokBaseUrl(baseUrl);
      updateGrokConfig((config) => {
        config.base_url = baseUrl.trim().replace(/\/+$/, "");
      });
    },
    [updateGrokConfig],
  );

  const handleGrokApiKeyChange = useCallback(
    (apiKey: string) => {
      setGrokApiKey(apiKey);
      updateGrokConfig((config) => {
        config.api_key = apiKey;
      });
    },
    [updateGrokConfig],
  );

  const handleGrokApiBackendChange = useCallback(
    (backend: GrokApiBackend) => {
      setGrokApiBackend(backend);
      updateGrokConfig((config) => {
        config.api_backend = backend;
      });
    },
    [updateGrokConfig],
  );

  const handleGrokModelChange = useCallback(
    (model: string) => {
      setGrokModel(model);
      updateGrokConfig((config) => {
        config.model = model;
      });
    },
    [updateGrokConfig],
  );

  const handleGrokUseLocalRouteChange = useCallback(
    (enabled: boolean) => {
      try {
        const config = JSON.parse(
          getSettingsConfig() || GROK_DEFAULT_CONFIG,
        ) as GrokProviderSettingsConfig;
        if (config.kind === "official") {
          setGrokUseLocalRoute(false);
          updateGrokConfig((cfg) => {
            cfg.useLocalRoute = false;
          });
          return;
        }
      } catch {
        // ignore
      }
      setGrokUseLocalRoute(enabled);
      updateGrokConfig((cfg) => {
        cfg.useLocalRoute = enabled;
      });
    },
    [getSettingsConfig, updateGrokConfig],
  );

  const handleGrokChatReasoningChange = useCallback(
    (value: GrokChatReasoning) => {
      setGrokChatReasoning(value);
      updateGrokConfig((config) => {
        config.grokChatReasoning = value;
      });
    },
    [updateGrokConfig],
  );

  const handleGrokDefaultReasoningEffortChange = useCallback(
    (effort: GrokReasoningEffort) => {
      setGrokDefaultReasoningEffort(effort);
      updateGrokConfig((config) => {
        config.defaultReasoningEffort = effort;
      });
    },
    [updateGrokConfig],
  );

  const handleGrokModelMappingsChange = useCallback(
    (mappings: GrokModelMapping[]) => {
      const normalized = normalizeMappings(mappings);
      // 保证至少有一个 isDefault
      if (normalized.length > 0 && !normalized.some((m) => m.isDefault)) {
        normalized[0] = { ...normalized[0], isDefault: true };
      }
      // 同步 default model
      const defaultMapping =
        normalized.find((m) => m.isDefault) ?? normalized[0];
      setGrokModelMappings(normalized);
      if (defaultMapping?.id) {
        setGrokModel(defaultMapping.id);
      }
      updateGrokConfig((config) => {
        config.modelMappings = normalized;
        if (defaultMapping?.id) {
          config.model = defaultMapping.id;
        }
        if (defaultMapping?.contextWindow) {
          config.context_window = defaultMapping.contextWindow;
        }
      });
    },
    [updateGrokConfig],
  );

  const resetGrokState = useCallback(
    (config?: Partial<GrokProviderSettingsConfig>) => {
      const kind = config?.kind ?? "custom";
      setGrokProviderKey("");
      setGrokBaseUrl(config?.base_url || "");
      setGrokApiKey(config?.api_key || "");
      setGrokApiBackend(config?.api_backend || "chat_completions");
      setGrokModel(config?.model || "");
      setGrokUseLocalRoute(
        kind === "official" ? false : config?.useLocalRoute === true,
      );
      setGrokChatReasoning(config?.grokChatReasoning ?? {});
      setGrokDefaultReasoningEffort(config?.defaultReasoningEffort || "medium");
      setGrokModelMappings(normalizeMappings(config?.modelMappings));
    },
    [],
  );

  const buildValidatedConfig = useCallback(() => {
    let parsed: Record<string, unknown> = {};
    try {
      parsed = JSON.parse(getSettingsConfig() || GROK_DEFAULT_CONFIG);
    } catch {
      parsed = { ...GROK_DEFAULT_CONFIG_OBJ };
    }

    const kind = parsed.kind === "official" ? "official" : "custom";
    const config: GrokProviderSettingsConfig = {
      ...parsed,
      kind,
      model: typeof parsed.model === "string" ? parsed.model : grokModel,
      base_url:
        typeof parsed.base_url === "string" ? parsed.base_url : grokBaseUrl,
      api_key: typeof parsed.api_key === "string" ? parsed.api_key : grokApiKey,
      api_backend:
        (parsed.api_backend as GrokApiBackend | undefined) || grokApiBackend,
      useLocalRoute: kind === "official" ? false : grokUseLocalRoute,
      modelMappings:
        (parsed.modelMappings as GrokModelMapping[] | undefined) ??
        grokModelMappings,
      grokChatReasoning:
        (parsed.grokChatReasoning as GrokChatReasoning | undefined) ??
        grokChatReasoning,
      defaultReasoningEffort:
        (parsed.defaultReasoningEffort as GrokReasoningEffort | undefined) ??
        grokDefaultReasoningEffort,
    };

    const validation = validateGrokSettingsConfig(config);
    if (!validation.ok) return validation;
    return { ok: true as const, config };
  }, [
    getSettingsConfig,
    grokApiBackend,
    grokApiKey,
    grokBaseUrl,
    grokChatReasoning,
    grokDefaultReasoningEffort,
    grokModel,
    grokModelMappings,
    grokUseLocalRoute,
  ]);

  return {
    grokProviderKey,
    setGrokProviderKey,
    grokBaseUrl,
    grokApiKey,
    grokApiBackend,
    grokModel,
    grokUseLocalRoute,
    grokChatReasoning,
    grokDefaultReasoningEffort,
    grokModelMappings,
    existingGrokKeys,
    handleGrokBaseUrlChange,
    handleGrokApiKeyChange,
    handleGrokApiBackendChange,
    handleGrokModelChange,
    handleGrokUseLocalRouteChange,
    handleGrokChatReasoningChange,
    handleGrokDefaultReasoningEffortChange,
    handleGrokModelMappingsChange,
    resetGrokState,
    buildValidatedConfig,
  };
}
