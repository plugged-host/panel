import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import { useToast } from '@/providers/ToastProvider.tsx';

interface UseResourceOptions<T> {
  queryKey: readonly unknown[];
  queryFn: () => Promise<T>;
  enabled?: boolean;
  silent?: boolean;
  keepPrevious?: boolean;
  /** Poll this often (ms); the lists use it so new replies and neo answers appear without a refresh. */
  refetchInterval?: number;
}

export function useResource<T>({
  queryKey,
  queryFn,
  enabled,
  silent = false,
  keepPrevious = false,
  refetchInterval,
}: UseResourceOptions<T>) {
  const { addToast } = useToast();
  const queryClient = useQueryClient();

  const { data, isFetching, error, refetch } = useQuery({
    queryKey,
    queryFn,
    enabled,
    placeholderData: keepPrevious ? keepPreviousData : undefined,
    refetchInterval,
  });

  useEffect(() => {
    if (error && !silent) addToast(httpErrorToHuman(error), 'error');
  }, [error]);

  return {
    data,
    loading: isFetching,
    error,
    refetch: () => refetch(),
    invalidate: () => queryClient.invalidateQueries({ queryKey }),
  };
}
