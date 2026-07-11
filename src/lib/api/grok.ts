import { invoke } from "@tauri-apps/api/core";

export interface GrokAuthStatus {
  loggedIn: boolean;
  email?: string | null;
  expired: boolean;
}

/**
 * Grok CLI surface for CC Switch.
 *
 * Deep Grok configuration stays in ~/.grok/config.toml; CC Switch manages
 * providers, local-route takeover, and a lightweight auth/status readout.
 */
export const grokApi = {
  async getAuthStatus(): Promise<GrokAuthStatus> {
    return await invoke("get_grok_auth_status");
  },

  async getLiveProviderIds(): Promise<string[]> {
    return await invoke("get_grok_live_provider_ids");
  },

  async getLiveProvider(
    providerId: string,
  ): Promise<Record<string, unknown> | null> {
    return await invoke("get_grok_live_provider", { providerId });
  },

  async importFromLive(): Promise<number> {
    return await invoke("import_grok_providers_from_live");
  },
};
