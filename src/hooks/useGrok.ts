import {
  useMutation,
  useQuery,
  useQueryClient,
  type QueryClient,
} from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { grokApi } from "@/lib/api/grok";
import { extractErrorMessage } from "@/utils/errorUtils";

export const grokKeys = {
  all: ["grok"] as const,
  liveProviderIds: ["grok", "liveProviderIds"] as const,
  authStatus: ["grok", "authStatus"] as const,
};

export function invalidateGrokProviderCaches(queryClient: QueryClient) {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: grokKeys.liveProviderIds }),
    queryClient.invalidateQueries({ queryKey: grokKeys.authStatus }),
  ]);
}

export function useGrokLiveProviderIds(enabled: boolean) {
  return useQuery({
    queryKey: grokKeys.liveProviderIds,
    queryFn: () => grokApi.getLiveProviderIds(),
    enabled,
  });
}

export function useGrokAuthStatus(enabled: boolean) {
  return useQuery({
    queryKey: grokKeys.authStatus,
    queryFn: () => grokApi.getAuthStatus(),
    enabled,
    staleTime: 30_000,
  });
}

export function useImportGrokProviders() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  return useMutation({
    mutationFn: () => grokApi.importFromLive(),
    onSuccess: async (count) => {
      await invalidateGrokProviderCaches(queryClient);
      await queryClient.invalidateQueries({ queryKey: ["providers", "grok"] });
      if (count > 0) {
        toast.success(
          t("provider.importCurrentDescription", {
            defaultValue: "已从当前配置导入",
          }),
        );
      } else {
        toast.info(
          t("provider.noProviders", { defaultValue: "未发现可导入的供应商" }),
        );
      }
    },
    onError: (error) => {
      toast.error(extractErrorMessage(error) || String(error));
    },
  });
}
