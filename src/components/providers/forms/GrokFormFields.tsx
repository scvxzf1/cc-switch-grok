import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { FormLabel } from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { ChevronDown, ChevronRight, Plus, Trash2 } from "lucide-react";
import { ApiKeySection } from "./shared";
import type {
  GrokApiBackend,
  GrokChatReasoning,
  GrokModelMapping,
} from "@/config/grokProviderPresets";
import type { ProviderCategory } from "@/types";
import {
  GROK_API_BACKENDS,
  GROK_REASONING_EFFORTS,
  applyGrokReasoningToggle,
  type GrokReasoningEffort,
} from "./hooks/useGrokFormState";

export interface GrokFormFieldsProps {
  baseUrl: string;
  onBaseUrlChange: (value: string) => void;
  apiKey: string;
  onApiKeyChange: (value: string) => void;
  apiBackend: GrokApiBackend;
  onApiBackendChange: (value: GrokApiBackend) => void;
  model: string;
  onModelChange: (value: string) => void;
  useLocalRoute: boolean;
  onUseLocalRouteChange: (value: boolean) => void;
  grokChatReasoning?: GrokChatReasoning;
  onGrokChatReasoningChange: (value: GrokChatReasoning) => void;
  defaultReasoningEffort: GrokReasoningEffort;
  onDefaultReasoningEffortChange: (value: GrokReasoningEffort) => void;
  modelMappings: GrokModelMapping[];
  onModelMappingsChange: (mappings: GrokModelMapping[]) => void;
  category?: ProviderCategory;
  shouldShowApiKeyLink: boolean;
  websiteUrl: string;
  isPartner?: boolean;
  partnerPromotionKey?: string;
}

export function GrokFormFields({
  baseUrl,
  onBaseUrlChange,
  apiKey,
  onApiKeyChange,
  apiBackend,
  onApiBackendChange,
  model,
  onModelChange,
  useLocalRoute,
  onUseLocalRouteChange,
  grokChatReasoning = {},
  onGrokChatReasoningChange,
  defaultReasoningEffort,
  onDefaultReasoningEffortChange,
  modelMappings,
  onModelMappingsChange,
  category,
  shouldShowApiKeyLink,
  websiteUrl,
  isPartner,
  partnerPromotionKey,
}: GrokFormFieldsProps) {
  const { t } = useTranslation();
  const isOfficial = category === "official";

  const supportsThinking =
    grokChatReasoning.supportsThinking === true ||
    grokChatReasoning.supportsEffort === true;
  const supportsEffort = grokChatReasoning.supportsEffort === true;

  const hasAnyAdvancedValue =
    useLocalRoute ||
    supportsThinking ||
    supportsEffort ||
    modelMappings.length > 0 ||
    !!defaultReasoningEffort;
  const [advancedExpanded, setAdvancedExpanded] = useState<boolean>(hasAnyAdvancedValue);

  useEffect(() => {
    if (hasAnyAdvancedValue) {
      setAdvancedExpanded(true);
    }
  }, [hasAnyAdvancedValue]);

  const mappingKeysRef = useRef<string[]>([]);
  while (mappingKeysRef.current.length < modelMappings.length) {
    mappingKeysRef.current.push(crypto.randomUUID());
  }
  if (mappingKeysRef.current.length > modelMappings.length) {
    mappingKeysRef.current.length = modelMappings.length;
  }

  const handleThinkingChange = useCallback(
    (checked: boolean) => {
      onGrokChatReasoningChange(
        applyGrokReasoningToggle(grokChatReasoning, "thinking", checked),
      );
    },
    [grokChatReasoning, onGrokChatReasoningChange],
  );

  const handleEffortChange = useCallback(
    (checked: boolean) => {
      onGrokChatReasoningChange(
        applyGrokReasoningToggle(grokChatReasoning, "effort", checked),
      );
    },
    [grokChatReasoning, onGrokChatReasoningChange],
  );

  const handleAddMapping = () => {
    mappingKeysRef.current.push(crypto.randomUUID());
    onModelMappingsChange([
      ...modelMappings,
      {
        id: "",
        upstreamModel: "",
        displayName: "",
        isDefault: modelMappings.length === 0,
      },
    ]);
  };

  const handleRemoveMapping = (index: number) => {
    mappingKeysRef.current.splice(index, 1);
    const next = modelMappings.filter((_, i) => i !== index);
    if (next.length > 0 && !next.some((m) => m.isDefault)) {
      next[0] = { ...next[0], isDefault: true };
    }
    onModelMappingsChange(next);
  };

  const handleMappingChange = (
    index: number,
    field: keyof GrokModelMapping,
    value: unknown,
  ) => {
    const next = modelMappings.map((item, i) => {
      if (i !== index) {
        if (field === "isDefault" && value === true) {
          return { ...item, isDefault: false };
        }
        return item;
      }
      return { ...item, [field]: value };
    });
    onModelMappingsChange(next);
  };

  const defaultModelOptions = useMemo(
    () => modelMappings.filter((m) => m.id.trim()),
    [modelMappings],
  );

  return (
    <>
      <div className="space-y-2">
        <FormLabel htmlFor="grok-baseurl">
          {t("grok.form.baseUrl", { defaultValue: "API 端点" })}
        </FormLabel>
        <Input
          id="grok-baseurl"
          value={baseUrl}
          onChange={(e) => onBaseUrlChange(e.target.value)}
          placeholder="https://api.example.com/v1"
          disabled={isOfficial}
        />
        <p className="text-xs text-muted-foreground">
          {t("grok.form.baseUrlHint", {
            defaultValue: "自定义供应商的 API 端点。官方供应商无需填写。",
          })}
        </p>
      </div>

      <ApiKeySection
        value={apiKey}
        onChange={onApiKeyChange}
        category={isOfficial ? undefined : category}
        shouldShowLink={shouldShowApiKeyLink}
        websiteUrl={websiteUrl}
        isPartner={isPartner}
        partnerPromotionKey={partnerPromotionKey}
      />

      <div className="space-y-2">
        <FormLabel htmlFor="grok-api-backend">
          {t("grok.form.apiBackend", { defaultValue: "API 协议" })}
        </FormLabel>
        <Select
          value={apiBackend}
          onValueChange={(v) => onApiBackendChange(v as GrokApiBackend)}
          disabled={isOfficial}
        >
          <SelectTrigger id="grok-api-backend">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {GROK_API_BACKENDS.map((item) => (
              <SelectItem key={item.value} value={item.value}>
                {t(item.labelKey)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <p className="text-xs text-muted-foreground">
          {t("grok.form.apiBackendHint", {
            defaultValue: "选择上游兼容协议。多数 OpenAI 兼容代理使用 Chat Completions。",
          })}
        </p>
      </div>

      <div className="space-y-2">
        <FormLabel htmlFor="grok-default-model">
          {t("grok.form.defaultModel", { defaultValue: "默认模型" })}
        </FormLabel>
        {defaultModelOptions.length > 0 ? (
          <Select
            value={model || defaultModelOptions[0]?.id}
            onValueChange={(value) => {
              onModelChange(value);
              const next = modelMappings.map((item) => ({
                ...item,
                isDefault: item.id === value,
              }));
              onModelMappingsChange(next);
            }}
          >
            <SelectTrigger id="grok-default-model">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {defaultModelOptions.map((item) => (
                <SelectItem key={item.id} value={item.id}>
                  {item.displayName || item.id}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        ) : (
          <Input
            id="grok-default-model"
            value={model}
            onChange={(e) => onModelChange(e.target.value)}
            placeholder="grok-build"
          />
        )}
      </div>

      <Collapsible
        open={advancedExpanded}
        onOpenChange={setAdvancedExpanded}
        className="rounded-lg border border-border-default p-4"
      >
        <CollapsibleTrigger asChild>
          <Button
            type="button"
            variant={null}
            size="sm"
            className="h-8 w-full justify-start gap-1.5 px-0 text-sm font-medium text-foreground hover:opacity-70"
            aria-label={t("providerForm.advancedOptionsToggle", {
              defaultValue: "高级选项",
            })}
          >
            {advancedExpanded ? (
              <ChevronDown className="h-4 w-4" />
            ) : (
              <ChevronRight className="h-4 w-4" />
            )}
            {t("providerForm.advancedOptionsToggle", {
              defaultValue: "高级选项",
            })}
          </Button>
        </CollapsibleTrigger>
        {!advancedExpanded && (
          <p className="mt-1 ml-1 text-xs text-muted-foreground">
            {t("grok.form.advancedSectionHint", {
              defaultValue:
                "包含本地路由、思考能力、思考等级与模型映射。",
            })}
          </p>
        )}
        <CollapsibleContent className="space-y-3 pt-3">
          <div className="flex items-center justify-between gap-4">
            <div className="space-y-1">
              <FormLabel>
                {t("grok.form.localRoute", { defaultValue: "本地路由" })}
              </FormLabel>
              <p className="text-xs leading-relaxed text-muted-foreground">
                {isOfficial
                  ? t("grok.form.localRouteOfficialHint", {
                      defaultValue: "官方供应商固定直连，不能开启本地路由。",
                    })
                  : t("grok.form.localRouteHint", {
                      defaultValue:
                        "开启后由本地代理转发请求，可用于协议转换与模型映射。",
                    })}
              </p>
            </div>
            <Switch
              checked={isOfficial ? false : useLocalRoute}
              disabled={isOfficial}
              onCheckedChange={onUseLocalRouteChange}
              aria-label={t("grok.form.localRoute", {
                defaultValue: "本地路由",
              })}
            />
          </div>

          <div className="flex items-center justify-between gap-4 border-t border-border-default pt-3">
            <div className="space-y-1">
              <FormLabel>
                {t("grok.form.reasoningModeToggle", {
                  defaultValue: "支持思考模式",
                })}
              </FormLabel>
              <p className="text-xs leading-relaxed text-muted-foreground">
                {t("grok.form.reasoningModeHint", {
                  defaultValue:
                    "上游支持开启/关闭 thinking 时启用。",
                })}
              </p>
            </div>
            <Switch
              checked={supportsThinking}
              onCheckedChange={handleThinkingChange}
              aria-label={t("grok.form.reasoningModeToggle", {
                defaultValue: "支持思考模式",
              })}
            />
          </div>

          <div className="flex items-center justify-between gap-4 border-t border-border-default pt-3">
            <div className="space-y-1">
              <FormLabel>
                {t("grok.form.reasoningEffortToggle", {
                  defaultValue: "支持思考等级",
                })}
              </FormLabel>
              <p className="text-xs leading-relaxed text-muted-foreground">
                {t("grok.form.reasoningEffortHint", {
                  defaultValue:
                    "上游支持 low/high/max 等思考深度时启用。开启后会自动打开思考模式。",
                })}
              </p>
            </div>
            <Switch
              checked={supportsEffort}
              onCheckedChange={handleEffortChange}
              aria-label={t("grok.form.reasoningEffortToggle", {
                defaultValue: "支持思考等级",
              })}
            />
          </div>

          {supportsEffort && (
            <div className="space-y-1.5 border-t border-border-default pt-3">
              <FormLabel htmlFor="grok-default-effort">
                {t("grok.form.defaultReasoningEffort", {
                  defaultValue: "默认思考等级",
                })}
              </FormLabel>
              <Select
                value={defaultReasoningEffort}
                onValueChange={(v) =>
                  onDefaultReasoningEffortChange(v as GrokReasoningEffort)
                }
              >
                <SelectTrigger id="grok-default-effort">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {GROK_REASONING_EFFORTS.map((effort) => (
                    <SelectItem key={effort} value={effort}>
                      {effort}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          )}

          <div className="space-y-3 border-t border-border-default pt-3">
            <div className="flex items-center justify-between gap-3">
              <div className="space-y-1">
                <FormLabel>
                  {t("grok.form.modelMappingTitle", {
                    defaultValue: "模型映射",
                  })}
                </FormLabel>
                <p className="text-xs leading-relaxed text-muted-foreground">
                  {t("grok.form.modelMappingHint", {
                    defaultValue:
                      "配置显示名、本地模型 ID、上游模型与上下文窗口。至少保留一行。",
                  })}
                </p>
              </div>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-7 gap-1"
                onClick={handleAddMapping}
              >
                <Plus className="h-3.5 w-3.5" />
                {t("grok.form.addModel", { defaultValue: "添加模型" })}
              </Button>
            </div>

            {modelMappings.length === 0 ? (
              <p className="text-sm text-muted-foreground py-2">
                {t("grok.form.noModels", {
                  defaultValue: "暂无模型映射，请至少添加一个模型。",
                })}
              </p>
            ) : (
              <div className="space-y-3">
                {modelMappings.map((mapping, index) => (
                  <div
                    key={mappingKeysRef.current[index]}
                    className="space-y-2 rounded-lg border border-border/50 p-3"
                  >
                    <div className="grid grid-cols-1 gap-2 md:grid-cols-2">
                      <div className="space-y-1">
                        <label className="text-xs text-muted-foreground">
                          {t("grok.form.displayName", {
                            defaultValue: "显示名",
                          })}
                        </label>
                        <Input
                          value={mapping.displayName ?? ""}
                          onChange={(e) =>
                            handleMappingChange(
                              index,
                              "displayName",
                              e.target.value,
                            )
                          }
                          placeholder="Grok Build"
                        />
                      </div>
                      <div className="space-y-1">
                        <label className="text-xs text-muted-foreground">
                          {t("grok.form.modelId", {
                            defaultValue: "模型 ID",
                          })}
                        </label>
                        <Input
                          value={mapping.id}
                          onChange={(e) =>
                            handleMappingChange(index, "id", e.target.value)
                          }
                          placeholder="grok-build"
                        />
                      </div>
                      <div className="space-y-1">
                        <label className="text-xs text-muted-foreground">
                          {t("grok.form.upstreamModel", {
                            defaultValue: "上游模型",
                          })}
                        </label>
                        <Input
                          value={mapping.upstreamModel}
                          onChange={(e) =>
                            handleMappingChange(
                              index,
                              "upstreamModel",
                              e.target.value,
                            )
                          }
                          placeholder="grok-build"
                        />
                      </div>
                      <div className="space-y-1">
                        <label className="text-xs text-muted-foreground">
                          {t("grok.form.contextWindow", {
                            defaultValue: "上下文窗口",
                          })}
                        </label>
                        <Input
                          type="number"
                          min={1}
                          value={mapping.contextWindow ?? ""}
                          onChange={(e) => {
                            const raw = e.target.value.replace(/[^\d]/g, "");
                            handleMappingChange(
                              index,
                              "contextWindow",
                              raw ? Number.parseInt(raw, 10) : undefined,
                            );
                          }}
                          placeholder="128000"
                        />
                      </div>
                    </div>
                    <div className="flex items-center justify-between gap-3">
                      <label className="flex items-center gap-2 text-xs text-muted-foreground">
                        <input
                          type="checkbox"
                          checked={mapping.isDefault === true}
                          onChange={(e) =>
                            handleMappingChange(
                              index,
                              "isDefault",
                              e.target.checked,
                            )
                          }
                        />
                        {t("grok.form.isDefault", {
                          defaultValue: "设为默认",
                        })}
                      </label>
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-7 w-7 p-0 text-muted-foreground"
                        onClick={() => handleRemoveMapping(index)}
                        aria-label={t("grok.form.removeModel", {
                          defaultValue: "删除模型",
                        })}
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </CollapsibleContent>
      </Collapsible>
    </>
  );
}
