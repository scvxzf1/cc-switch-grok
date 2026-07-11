# Grok App Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 CC Switch 中新增完整的 `Grok` 应用支持，可管理官方/第三方供应商，安全写回 `~/.grok/config.toml`，并在高级选项中支持本地路由、思考能力、思考等级、模型映射与上下文设置。

**Architecture:** 以 Hermes 累加式供应商为模板新增 `AppType::Grok`；用 `toml_edit` 精确改写 `~/.grok/config.toml` 的 `[models].default` 与 `[model.<id>]`；第三方高级能力走现有本地路由代理做 model rewrite 与 thinking/effort 注入。第一期不做 MCP/Skills/会话深度接入。

**Tech Stack:** Tauri 2 + React 18 + TypeScript + Rust；`toml`/`toml_edit`；现有 ProviderService / ProxyService / SQLite providers 表；Vitest + `cargo test`。

## Global Constraints

- 源码目录：`/home/scv/cc-switch-build`
- 基线版本：`3.16.5`（提交 `f39d463c` 之后的本地 docs 提交可保留）
- 应用 ID 固定为 `grok`；枚举名 `AppType::Grok`
- 默认配置目录：`~/.grok`；主文件：`config.toml`；登录态：`auth.json`（只读）
- 官方供应商禁止本地路由
- TOML 只改目标键，不得重写整文件抹掉 `[ui]` / `[marketplace]` 等用户段
- 写配置前必须备份到 `~/.cc-switch/backups/grok/`（或项目现有 backup 根目录约定）
- 不向 Grok TOML 写入未知托管标记字段（第一期）
- 第一期不做 MCP / Skills / 会话深度接入 / 自动 `grok login`
- 所有用户可见文案至少补 `zh` / `zh-TW` / `en` / `ja`
- 提交信息使用英文 conventional commits
- 测试优先：每个任务先写失败测试，再最小实现

## File Structure

### 新建

| 文件 | 职责 |
|---|---|
| `src-tauri/src/grok_config.rs` | Grok 路径、TOML 读写、provider upsert/remove、switch default、auth 只读、备份 |
| `src/config/grokProviderPresets.ts` | 官方/第三方预设与类型 |
| `src/components/providers/forms/GrokFormFields.tsx` | Grok 表单基础区 + 高级区 |
| `src/components/providers/forms/hooks/useGrokFormState.ts` | 表单状态与 settingsConfig 同步 |
| `src/lib/api/grok.ts` | 前端 API：live ids、auth 状态、可选 import |
| `src/hooks/useGrok.ts` | React Query hooks |
| `src-tauri/tests` 内相关用例 或 `grok_config.rs` 模块测 | 后端单测 |
| `tests/components/GrokFormFields.test.tsx` | 前端表单测 |
| `tests/config/grokProviderPresets.test.ts` | 预设测 |

### 主要修改

| 文件 | 改动 |
|---|---|
| `src-tauri/src/app_config.rs` | `AppType::Grok`、additive、all、from_str、默认 apps |
| `src-tauri/src/lib.rs` | `mod grok_config`、命令注册、启动 import（如有） |
| `src-tauri/src/settings.rs` | `grok_config_dir` / getter |
| `src-tauri/src/services/provider/mod.rs` | switch/delete/common-config 分支 |
| `src-tauri/src/services/provider/live.rs` | live read/write/import/remove |
| `src-tauri/src/services/proxy.rs` | Grok takeover 读写 live 配置 |
| `src-tauri/src/proxy/**` | Grok/OpenAI-compatible 请求最小转换（model/thinking） |
| `src-tauri/src/commands/**` | hermes 平行命令与 misc 安装检测 |
| `src/config/appConfig.tsx` | APP_IDS / 图标样式 |
| `src/App.tsx` | 应用切换、默认可见 |
| `src/types.ts` / `src/lib/api/types.ts` | AppId、VisibleApps、settings 字段 |
| `src/components/providers/forms/ProviderForm.tsx` | 分发到 Grok 表单 |
| `src/components/settings/*` | 目录覆盖与可见性 |
| `src/i18n/locales/*` | 文案 |

---

### Task 1: Grok 配置核心（路径 + TOML 安全读写）

**Files:**
- Create: `src-tauri/src/grok_config.rs`
- Modify: `src-tauri/src/lib.rs`（`mod grok_config;`）
- Modify: `src-tauri/src/settings.rs`（`grok_config_dir` + `get_grok_override_dir`）
- Test: `src-tauri/src/grok_config.rs` 内 `#[cfg(test)]`

**Interfaces:**
- Produces:
  - `pub fn get_grok_dir() -> PathBuf`
  - `pub fn get_grok_config_path() -> PathBuf`
  - `pub fn get_grok_auth_path() -> PathBuf`
  - `pub fn read_config_document() -> Result<toml_edit::DocumentMut, AppError>`
  - `pub fn write_config_document(doc: &toml_edit::DocumentMut) -> Result<GrokWriteOutcome, AppError>`
  - `pub struct GrokWriteOutcome { pub backup_path: Option<String> }`

- [ ] **Step 1: 写失败测试（路径覆盖 + 只改目标键）**

在 `grok_config.rs` 底部加入测试模块，覆盖：

```rust
#[test]
fn get_grok_dir_uses_settings_override() { /* set test home + settings.grok_config_dir */ }

#[test]
fn write_default_model_preserves_unrelated_sections() {
    // seed config.toml with [ui] and [marketplace]
    // set [models].default = "proxy-1"
    // assert [ui] yolo 与 marketplace 仍在
}
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```bash
cd /home/scv/cc-switch-build/src-tauri && cargo test grok_config -- --nocapture
```

Expected: 编译失败或测试失败（模块/函数不存在）。

- [ ] **Step 3: 最小实现**

实现要点：

```rust
pub fn get_grok_dir() -> PathBuf {
    if let Some(override_dir) = crate::settings::get_grok_override_dir() {
        return override_dir;
    }
    crate::config::get_home_dir().join(".grok")
}

pub fn get_grok_config_path() -> PathBuf {
    get_grok_dir().join("config.toml")
}
```

- 用 `toml_edit::DocumentMut` 读写，保留注释与顺序
- `write_config_document`：备份 -> 原子写
- settings 增加：

```rust
pub grok_config_dir: Option<String>,
pub fn get_grok_override_dir() -> Option<PathBuf> { /* 同 hermes */ }
```

- [ ] **Step 4: 跑通测试**

Run:

```bash
cd /home/scv/cc-switch-build/src-tauri && cargo test grok_config -- --nocapture
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/grok_config.rs src-tauri/src/lib.rs src-tauri/src/settings.rs
git commit -m "feat(grok): add config path and safe TOML read/write"
```

---

### Task 2: Grok provider live 操作（upsert/remove/switch/import）

**Files:**
- Modify: `src-tauri/src/grok_config.rs`
- Test: `src-tauri/src/grok_config.rs`

**Interfaces:**
- Consumes: Task 1 的 document 读写
- Produces:
  - `pub fn get_models() -> Result<serde_json::Map<String, Value>, AppError>`
  - `pub fn set_model(id: &str, settings: Value) -> Result<GrokWriteOutcome, AppError>`
  - `pub fn remove_model(id: &str) -> Result<GrokWriteOutcome, AppError>`
  - `pub fn set_default_model(id: &str) -> Result<GrokWriteOutcome, AppError>`
  - `pub fn apply_switch_defaults(provider_id: &str, settings: &Value) -> Result<GrokWriteOutcome, AppError>`
  - `pub fn read_auth_status() -> Result<GrokAuthStatus, AppError>`

`GrokAuthStatus`：

```rust
pub struct GrokAuthStatus {
    pub logged_in: bool,
    pub email: Option<String>,
    pub expired: bool,
}
```

- [ ] **Step 1: 写失败测试**

```rust
#[test]
fn set_model_writes_model_table_and_keeps_ui() { /* ... */ }

#[test]
fn apply_switch_defaults_sets_models_default() { /* ... */ }

#[test]
fn remove_model_deletes_only_target_table() { /* ... */ }

#[test]
fn read_auth_status_redacts_to_email_and_expiry() { /* fixture auth.json */ }
```

- [ ] **Step 2: 运行确认失败**

```bash
cd /home/scv/cc-switch-build/src-tauri && cargo test grok_config::tests -- --nocapture
```

- [ ] **Step 3: 实现 live 操作**

字段映射（直连写入 `[model.<id>]`）：

- `model` <- `settings.model`
- `base_url` <- `settings.base_url`
- `api_key` <- `settings.api_key`（空则不写或写空，按 Grok 文档兼容性选更安全者；默认：空字符串不写键）
- `api_backend` <- `settings.api_backend`
- `name` <- `settings.name`
- `context_window` <- `settings.context_window` 或默认映射行

`apply_switch_defaults`：

1. 若 `kind == official`：`set_default_model("grok-build")` 或 settings.model
2. 若 custom 且 `useLocalRoute != true`：upsert model 段 + set default 为 provider id/默认映射 id
3. 若 `useLocalRoute == true`：此任务只保证 default/元数据准备；实际 proxy base_url 在 Task 6 写

官方删除：`remove_model` 对 official 内建模型 no-op 或拒绝删除内建，只删 CCS 侧记录（service 层处理）。

- [ ] **Step 4: 测试通过**

```bash
cd /home/scv/cc-switch-build/src-tauri && cargo test grok_config -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/grok_config.rs
git commit -m "feat(grok): add model upsert/switch/auth status helpers"
```

---

### Task 3: 接入 AppType::Grok 与 ProviderService 主路径

**Files:**
- Modify: `src-tauri/src/app_config.rs`
- Modify: `src-tauri/src/services/provider/mod.rs`
- Modify: `src-tauri/src/services/provider/live.rs`
- Modify: `src-tauri/src/commands/provider.rs` 或现有 hermes 平行命令文件
- Modify: `src-tauri/src/lib.rs`（export/import 命令）
- Test: `src-tauri/src/services/provider/mod.rs` 现有测试风格新增 grok 用例；或 `src-tauri/tests/provider_service.rs`

**Interfaces:**
- Consumes: Task 2 live helpers
- Produces:
  - `AppType::Grok` / `"grok"`
  - `import_grok_providers_from_live(state) -> Result<usize, AppError>`
  - `remove_grok_provider_from_live(id) -> Result<(), AppError>`
  - switch 时调用 `grok_config::apply_switch_defaults`

- [ ] **Step 1: 写失败测试**

参考 Hermes：

```rust
#[test]
fn import_grok_providers_from_live_updates_existing_provider_from_live() { /* ... */ }
```

并确保 `AppType::from_str("grok") == Ok(AppType::Grok)`。

- [ ] **Step 2: 运行失败**

```bash
cd /home/scv/cc-switch-build/src-tauri && cargo test grok -- --nocapture
```

- [ ] **Step 3: 最小接入**

`app_config.rs`：

```rust
pub enum AppType { /* ... */ Grok }
// as_str => "grok"
// is_additive_mode includes Grok
// all() includes Grok
// from_str accepts "grok"
// default apps.insert("grok", ...)
```

`live.rs`：

- `get_live_providers` / write / remove 分支加 Grok
- `import_grok_providers_from_live` 平行 Hermes

`provider/mod.rs` switch/delete 分支加 Grok。

第一期 MCP/Skills 分支：显式 `false` / ignore，不要 panic。

所有穷尽匹配必须补全，否则不让编译过。

- [ ] **Step 4: 测试 + check**

```bash
cd /home/scv/cc-switch-build/src-tauri && cargo test grok -- --nocapture && cargo check
```

Expected: PASS / check OK

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/app_config.rs src-tauri/src/services/provider src-tauri/src/commands src-tauri/src/lib.rs src-tauri/tests
git commit -m "feat(grok): wire AppType and provider service live sync"
```

---

### Task 4: 前端应用骨架（入口、类型、设置目录、预设）

**Files:**
- Create: `src/config/grokProviderPresets.ts`
- Modify: `src/config/appConfig.tsx`
- Modify: `src/lib/api/types.ts`
- Modify: `src/types.ts`
- Modify: `src/App.tsx`
- Modify: `src/components/settings/DirectorySettings.tsx`
- Modify: `src/components/settings/AppVisibilitySettings.tsx`
- Modify: `src/hooks/useDirectorySettings.ts`
- Modify: `src/lib/schemas/settings.ts`（若有）
- Modify: `src/i18n/locales/zh.json` `zh-TW.json` `en.json` `ja.json`
- Test: `tests/config/grokProviderPresets.test.ts`

**Interfaces:**
- Produces:
  - `export type AppId = ... | "grok"`
  - `APP_IDS` 含 `grok`
  - `grokProviderPresets` 至少含官方 xAI
  - settings: `grokConfigDir?: string`
  - visibleApps.grok

- [ ] **Step 1: 写失败测试**

```ts
import { grokProviderPresets } from "@/config/grokProviderPresets";

test("includes official xAI preset with grok-build default", () => {
  const official = grokProviderPresets.find((p) => p.isOfficial);
  expect(official).toBeTruthy();
  expect(official!.settingsConfig.kind).toBe("official");
  expect(official!.settingsConfig.model).toBe("grok-build");
  expect(official!.settingsConfig.useLocalRoute).toBeFalsy();
});
```

- [ ] **Step 2: 跑测试失败**

```bash
cd /home/scv/cc-switch-build && pnpm test:unit tests/config/grokProviderPresets.test.ts
```

- [ ] **Step 3: 实现骨架**

`grokProviderPresets.ts` 关键类型：

```ts
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
}
```

官方预设：

```ts
{
  name: "xAI Official",
  isOfficial: true,
  websiteUrl: "https://grok.com",
  settingsConfig: {
    kind: "official",
    model: "grok-build",
    name: "Grok Build",
    useLocalRoute: false,
    modelMappings: [
      { id: "grok-build", upstreamModel: "grok-build", displayName: "Grok Build", isDefault: true }
    ],
  },
}
```

再加 1 个自定义中转模板（空 key + chat_completions）方便手工测。

前端 `AppId` / `APP_IDS` / `visibleApps` 默认 `grok: true`。  
图标可先复用现有 `ProviderIcon` 占位（如 `xai` / 文本 G），不要阻塞功能。

- [ ] **Step 4: 测试与 typecheck**

```bash
cd /home/scv/cc-switch-build && pnpm test:unit tests/config/grokProviderPresets.test.ts && pnpm typecheck
```

- [ ] **Step 5: Commit**

```bash
git add src/config/grokProviderPresets.ts src/config/appConfig.tsx src/lib/api/types.ts src/types.ts src/App.tsx src/components/settings src/hooks/useDirectorySettings.ts src/lib/schemas src/i18n tests/config/grokProviderPresets.test.ts
git commit -m "feat(grok): add frontend app shell, presets, and settings fields"
```

---

### Task 5: Grok 表单与 ProviderForm 分发（含高级选项 UI）

**Files:**
- Create: `src/components/providers/forms/hooks/useGrokFormState.ts`
- Create: `src/components/providers/forms/GrokFormFields.tsx`
- Modify: `src/components/providers/forms/ProviderForm.tsx`
- Modify: `src/components/providers/AddProviderDialog.tsx`（若需）
- Modify: i18n
- Test: `tests/components/GrokFormFields.test.tsx`

**Interfaces:**
- Consumes: `GrokProviderSettingsConfig`
- Produces: `useGrokFormState()` 与 `GrokFormFields`；保存时写入完整 settingsConfig

- [ ] **Step 1: 写失败测试**

覆盖：

1. 官方隐藏/禁用本地路由
2. 打开思考等级会连带打开思考能力
3. 模型映射为空时校验失败（若校验在 hook/纯函数，测纯函数）

示例：

```ts
test("enabling effort forces thinking on", () => {
  const next = applyGrokReasoningToggle({ supportsThinking: false, supportsEffort: false }, "effort", true);
  expect(next.supportsThinking).toBe(true);
  expect(next.supportsEffort).toBe(true);
});
```

- [ ] **Step 2: 跑失败**

```bash
cd /home/scv/cc-switch-build && pnpm test:unit tests/components/GrokFormFields.test.tsx
```

- [ ] **Step 3: 实现表单**

`useGrokFormState` 平行 Hermes：维护

- providerKey/id
- baseUrl / apiKey / apiBackend / model
- useLocalRoute
- grokChatReasoning
- modelMappings
- defaultReasoningEffort

`GrokFormFields` 布局：

**基础区**
- Base URL
- API Key
- API Backend select
- Default model

**高级区**
- 本地路由开关（官方 disabled + 提示）
- 思考能力开关
- 思考等级开关
- 模型映射表（显示名 / id / upstream / context / default）
- 增删行

`ProviderForm.tsx` 增加 `appId === "grok"` 分支：预设列表、校验、payload.providerKey、渲染 `GrokFormFields`。

保存校验：

- custom 需要合法 http(s) base_url（直连）或在本地路由模式下允许本地 proxy URL
- `modelMappings.length >= 1`
- official 强制 `useLocalRoute=false`

- [ ] **Step 4: 测试通过**

```bash
cd /home/scv/cc-switch-build && pnpm test:unit tests/components/GrokFormFields.test.tsx && pnpm typecheck
```

- [ ] **Step 5: Commit**

```bash
git add src/components/providers/forms src/i18n tests/components/GrokFormFields.test.tsx
git commit -m "feat(grok): add provider form with Codex-like advanced options"
```

---

### Task 6: 本地路由 takeover + 最小请求转换

**Files:**
- Modify: `src-tauri/src/services/proxy.rs`
- Modify: `src-tauri/src/proxy/handlers.rs` 或新建 `src-tauri/src/proxy/providers/grok.rs`
- Modify: `src-tauri/src/proxy/providers/mod.rs`
- Modify: `src-tauri/src/services/provider/mod.rs`（switch 时启用 takeover）
- Modify: `src-tauri/src/grok_config.rs`（写入本地 base_url）
- Test: proxy/provider 相关单测

**Interfaces:**
- Consumes: `settings.useLocalRoute` / `modelMappings` / `grokChatReasoning`
- Produces:
  - Grok live takeover：把当前入口 model 的 `base_url` 指到本地代理
  - 代理请求：rewrite model、注入 thinking/effort、带上游 Authorization

- [ ] **Step 1: 写失败测试**

```rust
#[test]
fn grok_takeover_rewrites_base_url_to_local_proxy() { /* ... */ }

#[test]
fn grok_proxy_rewrites_model_and_injects_reasoning_effort() { /* ... */ }

#[test]
fn official_provider_rejects_local_route() { /* ... */ }
```

- [ ] **Step 2: 跑失败**

```bash
cd /home/scv/cc-switch-build/src-tauri && cargo test grok -- --nocapture
```

- [ ] **Step 3: 最小实现**

实现策略：

1. `ProxyTakeoverStatus` / app enable 列表纳入 `grok`（若当前结构只写死 claude/codex/gemini，扩展字段时保持向后兼容默认 false）
2. switch 到 `useLocalRoute=true` 的 Grok 供应商时：
   - 确保 proxy 启动（复用 `start_with_takeover` 或 app-level enable）
   - 计算 `proxy_url`
   - 写 Grok `[model.<defaultId>]`：
     - `base_url = proxy_url`
     - `model = mapping.id`
     - `api_backend = chat_completions`（第一期默认）
     - **不要**把真实上游 key 写进 Grok；改由代理持有
3. 代理收到请求后：
   - 按当前 grok provider meta 找 mapping：`request.model -> upstreamModel`
   - 按 `grokChatReasoning` 注入 thinking/effort
   - 转发到真实 `base_url`
4. 关闭本地路由或切回官方：恢复直连配置 / 取消 grok takeover

第一期协议范围：OpenAI Chat Completions 为主；`messages`/`responses` 能透传最好，不能则明确报错。

- [ ] **Step 4: 测试通过**

```bash
cd /home/scv/cc-switch-build/src-tauri && cargo test grok -- --nocapture && cargo check
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/proxy.rs src-tauri/src/proxy src-tauri/src/services/provider src-tauri/src/grok_config.rs
git commit -m "feat(grok): add local route takeover and minimal request transforms"
```

---

### Task 7: CLI 检测、状态条、live import API、收尾验收

**Files:**
- Modify: `src-tauri/src/commands/misc.rs`（安装/版本检测名单加 grok）
- Create/Modify: `src/lib/api/grok.ts`、`src/hooks/useGrok.ts`
- Modify: `src/components/providers/ProviderList.tsx` / `ProviderCard.tsx` / `AboutSection.tsx`（如有工具列表）
- Modify: `src/App.tsx`（状态条数据接入，可先最小）
- Modify: i18n
- Test: 前端 hook/组件轻测 + 后端 misc 测（若已有模式）

**Interfaces:**
- Produces:
  - `get_grok_auth_status` command
  - `import_grok_providers_from_live` command
  - `get_grok_live_provider_ids` command
  - 前端 hooks：`useGrokAuthStatus`、`useImportGrokProviders`、`useGrokLiveProviderIds`

- [ ] **Step 1: 写失败测试/类型约束**

- misc 工具列表包含 `grok`
- API 模块导出函数存在（TS 测试或 typecheck 覆盖）

- [ ] **Step 2: 实现命令与 UI 接线**

`misc.rs`：

- 工具名 `grok`
- 检测 PATH / `~/.grok/bin/grok`
- 版本命令：`grok --version`

Provider 页打开时：

1. import live models
2. 显示 CLI 版本与登录态

- [ ] **Step 3: 全量相关验证**

```bash
cd /home/scv/cc-switch-build/src-tauri && cargo test grok -- --nocapture
cd /home/scv/cc-switch-build && pnpm test:unit tests/config/grokProviderPresets.test.ts tests/components/GrokFormFields.test.tsx
cd /home/scv/cc-switch-build && pnpm typecheck
```

手工验收清单：

1. 应用切换可见 Grok
2. 官方切换成功，`config.toml` 中 `[models].default` 正确
3. 添加第三方直连并切换成功
4. 开启本地路由 + 思考等级 + 映射后，settingsConfig 保存正确，live base_url 指向本地代理
5. 用户原有 `[ui]` 段仍在
6. 未登录官方时有提示，不崩溃

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands src/lib/api/grok.ts src/hooks/useGrok.ts src/components src/App.tsx src/i18n
git commit -m "feat(grok): add CLI detection, auth status, and live import UX"
```

- [ ] **Step 5: 最终提交（如有文档微调）**

```bash
git add docs/superpowers/plans/2026-07-12-grok-app-support.md
git commit -m "docs: mark grok implementation plan ready for execution" || true
```

---

## Spec Coverage Checklist

| 规格要求 | 对应任务 |
|---|---|
| 新增 AppType/Grok 页 | Task 3, 4 |
| 官方/第三方供应商管理 | Task 2, 3, 5 |
| 安全写 `config.toml` + 备份 | Task 1, 2 |
| live import | Task 2, 3, 7 |
| 本地路由映射 | Task 5 UI + Task 6 |
| 思考能力/思考等级 | Task 5 UI + Task 6 |
| 模型映射 + 上下文 | Task 5 UI + Task 2/6 写入 |
| CLI 检测与登录态 | Task 2 auth + Task 7 |
| `grokConfigDir` | Task 1 + Task 4 |
| 不做 MCP/Skills/会话 | Task 3 显式忽略 |
| 测试与 DoD | 每任务测试 + Task 7 验收 |

## Placeholder Scan

- 无 TBD/TODO 步骤
- 关键接口名已固定
- 每个任务含运行命令与提交

## Type Consistency

- app id 全程 `grok`
- settings 字段：`grokConfigDir`（前端）/ `grok_config_dir`（Rust）
- 高级字段名：`useLocalRoute`、`modelMappings`、`grokChatReasoning`
- live helper：`set_model` / `remove_model` / `apply_switch_defaults` / `read_auth_status`

## Execution Notes

- 推荐工作方式：subagent-driven，按 Task 1→7 顺序
- 若编译穷尽匹配爆炸，先只保证编译通过的 ignore 分支，再补行为
- 本地路由若发现现有 proxy 对非 Claude/Codex/Gemini 扩展成本过高，可在 Task 6 内先完成“写 meta + 写本地 base_url + 最小 forwarder”，但不得静默丢掉规格中的 model/thinking 转换
- 不要把 stash 中旧的 Linux Claude Desktop 改动自动弹回，除非单独确认
