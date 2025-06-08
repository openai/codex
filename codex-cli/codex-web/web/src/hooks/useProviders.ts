import useSWR from "swr";

const fetcher = (url: string) => fetch(url).then((r) => r.json());

export type Provider = {
  name: string;
  baseURL: string;
  envKey: string;
  manualModels?: unknown;
};

export function useProviders(): {
  providers: Record<string, Provider>;
  isLoading: boolean;
  error: unknown;
} {
  const { data, error } = useSWR("http://localhost:8787/providers", fetcher);

  return {
    providers: data?.providers ?? {},
    isLoading: !data && !error,
    error,
  } as const;
}
