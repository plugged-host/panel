import type { ReactNode } from 'react';
import { getHttpStatus, httpErrorToHuman } from '@/api/axios.ts';
import ErrorState from '@/elements/feedback/ErrorState.tsx';
import ScreenBlock from '@/elements/feedback/ScreenBlock.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

interface ResourceViewProps<T> {
  resource: {
    data: T | undefined;
    loading: boolean;
    error?: unknown;
    refetch?: () => void;
  };
  children: (data: T) => ReactNode;
}

export default function ResourceView<T>({ resource, children }: ResourceViewProps<T>) {
  const { t } = useTranslations();
  const { data, loading, error, refetch } = resource;

  if (data === undefined || data === null) {
    if (loading) {
      return <Spinner.Centered />;
    }

    if (error && getHttpStatus(error) !== 404) {
      return <ErrorState error={httpErrorToHuman(error)} onRetry={refetch} />;
    }

    return (
      <ScreenBlock
        title={t('elements.screenBlock.notFound.title', {})}
        content={t('elements.screenBlock.notFound.content', {})}
      />
    );
  }

  return <>{children(data)}</>;
}
