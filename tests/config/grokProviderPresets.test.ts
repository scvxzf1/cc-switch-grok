import { describe, expect, it } from "vitest";
import { grokProviderPresets } from "@/config/grokProviderPresets";

describe("grokProviderPresets", () => {
  it("includes official xAI preset with grok-build default", () => {
    const official = grokProviderPresets.find((p) => p.isOfficial);
    expect(official).toBeTruthy();
    expect(official!.settingsConfig.kind).toBe("official");
    expect(official!.settingsConfig.model).toBe("grok-build");
    expect(official!.settingsConfig.useLocalRoute).toBeFalsy();
    expect(official!.settingsConfig.modelMappings?.[0]?.id).toBe("grok-build");
    expect(official!.settingsConfig.modelMappings?.[0]?.isDefault).toBe(true);
  });

  it("includes a custom chat_completions template with empty api_key", () => {
    const custom = grokProviderPresets.find((p) => p.isCustomTemplate);
    expect(custom).toBeTruthy();
    expect(custom!.settingsConfig.kind).toBe("custom");
    expect(custom!.settingsConfig.api_backend).toBe("chat_completions");
    expect(custom!.settingsConfig.api_key).toBe("");
  });
});
