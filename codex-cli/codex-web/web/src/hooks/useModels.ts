import useSWR from "swr";

const fetcher = (url: string) => fetch(url).then((r) => r.json());

export type Model = { id: string; ctx: number };

export function useModels(provider?: string): {
  models: Array<Model>;
  isLoading: boolean;
  error: unknown;
} {
  const { data, error } = useSWR(
    provider ? `http://localhost:8787/providers/${provider}/models` : null,
    fetcher,
  );

  return {
    models: data?.models ?? [],
    isLoading: provider && !data && !error,
    error,
  } as const;
}
