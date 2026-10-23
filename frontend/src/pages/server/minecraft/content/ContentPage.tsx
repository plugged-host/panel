// plugged.host: the frame both Plugins & Mods tabs share (docs/features/minecraft.md)
import { Dispatch, ReactNode, SetStateAction } from 'react';
import { z } from 'zod';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Text from '@/elements/typography/Text.tsx';
import { minecraftContentOptionsSchema } from '@/lib/schemas/server/minecraft.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import ContentSubNavigation from './ContentSubNavigation.tsx';
import { minecraftLoaderLabels } from './labels.ts';
import { useContentOptions } from './useContentOptions.ts';

export type ContentOptions = z.infer<typeof minecraftContentOptionsSchema>;

interface Props {
  search?: string;
  setSearch?: Dispatch<SetStateAction<string>>;
  children: (options: ContentOptions) => ReactNode;
}

export default function ContentPage({ search, setSearch, children }: Props) {
  const { t } = useTranslations();
  const options = useContentOptions();
  const loader = options.data?.software.loader ?? null;

  return (
    <ServerContentContainer
      title={t('pages.server.minecraft.content.title', {})}
      subtitle={
        loader
          ? t('pages.server.minecraft.content.subtitle', {
              software: minecraftLoaderLabels[loader],
              version: options.data?.gameVersion ?? '?',
            })
          : undefined
      }
      search={loader ? search : undefined}
      setSearch={loader ? setSearch : undefined}
    >
      <ContentSubNavigation />

      <ResourceView resource={options}>
        {(options) =>
          options.software.loader ? (
            children(options)
          ) : (
            <Text c='dimmed'>{t('pages.server.minecraft.content.noLoader', {})}</Text>
          )
        }
      </ResourceView>
    </ServerContentContainer>
  );
}
