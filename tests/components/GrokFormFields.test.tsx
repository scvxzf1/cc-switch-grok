import { fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps, PropsWithChildren } from "react";
import { useForm } from "react-hook-form";
import { describe, expect, it, vi } from "vitest";
import {
  applyGrokReasoningToggle,
  validateGrokSettingsConfig,
} from "@/components/providers/forms/hooks/useGrokFormState";
import { GrokFormFields } from "@/components/providers/forms/GrokFormFields";
import { Form } from "@/components/ui/form";

type GrokFormFieldsProps = ComponentProps<typeof GrokFormFields>;

const FormShell = ({ children }: PropsWithChildren) => {
  const form = useForm();
  return <Form {...form}>{children}</Form>;
};

const renderGrokForm = (overrides: Partial<GrokFormFieldsProps> = {}) => {
  const props: GrokFormFieldsProps = {
    baseUrl: "https://api.example.com/v1",
    onBaseUrlChange: vi.fn(),
    apiKey: "sk-test",
    onApiKeyChange: vi.fn(),
    apiBackend: "chat_completions",
    onApiBackendChange: vi.fn(),
    model: "custom",
    onModelChange: vi.fn(),
    useLocalRoute: false,
    onUseLocalRouteChange: vi.fn(),
    grokChatReasoning: {
      supportsThinking: false,
      supportsEffort: false,
    },
    onGrokChatReasoningChange: vi.fn(),
    defaultReasoningEffort: "medium",
    onDefaultReasoningEffortChange: vi.fn(),
    modelMappings: [
      {
        id: "custom",
        upstreamModel: "custom",
        displayName: "Custom",
        isDefault: true,
      },
    ],
    onModelMappingsChange: vi.fn(),
    category: "custom",
    shouldShowApiKeyLink: false,
    websiteUrl: "",
    ...overrides,
  };

  return {
    props,
    ...render(
      <FormShell>
        <GrokFormFields {...props} />
      </FormShell>,
    ),
  };
};

describe("applyGrokReasoningToggle", () => {
  it("enabling effort forces thinking on", () => {
    const next = applyGrokReasoningToggle(
      { supportsThinking: false, supportsEffort: false },
      "effort",
      true,
    );
    expect(next.supportsThinking).toBe(true);
    expect(next.supportsEffort).toBe(true);
  });

  it("disabling thinking also disables effort", () => {
    const next = applyGrokReasoningToggle(
      { supportsThinking: true, supportsEffort: true },
      "thinking",
      false,
    );
    expect(next.supportsThinking).toBe(false);
    expect(next.supportsEffort).toBe(false);
  });
});

describe("validateGrokSettingsConfig", () => {
  it("rejects empty model mappings", () => {
    const result = validateGrokSettingsConfig({
      kind: "custom",
      model: "x",
      base_url: "https://api.example.com/v1",
      useLocalRoute: false,
      modelMappings: [],
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.errorKey).toBe("grok.form.modelMappingsRequired");
    }
  });

  it("forces official providers to keep local route disabled", () => {
    const result = validateGrokSettingsConfig({
      kind: "official",
      model: "grok-build",
      useLocalRoute: true,
      modelMappings: [
        {
          id: "grok-build",
          upstreamModel: "grok-build",
          isDefault: true,
        },
      ],
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.errorKey).toBe("grok.form.localRouteOfficialDisabled");
    }
  });

  it("requires http(s) base_url for custom direct mode", () => {
    const result = validateGrokSettingsConfig({
      kind: "custom",
      model: "x",
      base_url: "not-a-url",
      useLocalRoute: false,
      modelMappings: [{ id: "x", upstreamModel: "x", isDefault: true }],
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.errorKey).toBe("grok.form.baseUrlInvalid");
    }
  });
});

describe("GrokFormFields", () => {
  it("disables local route toggle for official providers", () => {
    renderGrokForm({
      category: "official",
      useLocalRoute: false,
    });

    const advanced = screen.getByRole("button", {
      name: /advanced options|高级选项/i,
    });
    if (advanced.getAttribute("aria-expanded") !== "true") {
      fireEvent.click(advanced);
    }

    const toggle = screen.getByRole("switch", {
      name: /local route|本地路由/i,
    });
    expect(toggle).toBeDisabled();
  });

  it("enabling effort toggle reports thinking+effort enabled", () => {
    const onGrokChatReasoningChange = vi.fn();
    renderGrokForm({
      category: "custom",
      onGrokChatReasoningChange,
      grokChatReasoning: {
        supportsThinking: false,
        supportsEffort: false,
      },
    });

    const advanced = screen.getByRole("button", {
      name: /advanced options|高级选项/i,
    });
    if (advanced.getAttribute("aria-expanded") !== "true") {
      fireEvent.click(advanced);
    }

    fireEvent.click(
      screen.getByRole("switch", {
        name: /reasoning effort|思考等级/i,
      }),
    );

    expect(onGrokChatReasoningChange).toHaveBeenCalledWith(
      expect.objectContaining({
        supportsThinking: true,
        supportsEffort: true,
      }),
    );
  });
});
