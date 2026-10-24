// plugged.host: the frame both Plugins & Mods tabs share (docs/features/minecraft.md)
import { Dispatch, ReactNode, SetStateAction, useState } from 'react';
import { z } from 'zod';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Text from '@/elements/typography/Text.tsx';
import { minecraftContentOptionsSchema } from '@/lib/schemas/server/minecraft.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import BedrockModal from './BedrockModal.tsx';
import ContentSubNavigation from './ContentSubNavigation.tsx';
import { minecraftLoaderLabels } from './labels.ts';
import { useContentOptions } from './useContentOptions.ts';

export type ContentOptions = z.infer<typeof minecraftContentOptionsSchema>;

// the loaders Geyser has a build for (shared/src/minecraft/bedrock.rs)
const BEDROCK_LOADERS = ['paper', 'fabric', 'neoforge'];

interface Props {
  search?: string;
  setSearch?: Dispatch<SetStateAction<string>>;
  // false: the tab works without a loader (a modpack brings its own)
  requireLoader?: boolean;
  children: (options: ContentOptions) => ReactNode;
}

export default function ContentPage({ search, setSearch, requireLoader = true, children }: Props) {
  const { t } = useTranslations();
  const options = useContentOptions();
  const loader = options.data?.software.loader ?? null;
  const canBedrock = useServerCan('minecraft.bedrock');
  const [bedrock, setBedrock] = useState(false);

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
      search={loader || !requireLoader ? search : undefined}
      setSearch={loader || !requireLoader ? setSearch : undefined}
      contentRight={
        canBedrock && loader && BEDROCK_LOADERS.includes(loader) ? (
          <Button variant='default' onClick={() => setBedrock(true)}>
            {t('pages.server.minecraft.bedrock.button.open', {})}
          </Button>
        ) : undefined
      }
    >
      <ContentSubNavigation />
      {canBedrock && <BedrockModal opened={bedrock} onClose={() => setBedrock(false)} />}

      <ResourceView resource={options}>
        {(options) =>
          options.software.loader || !requireLoader ? (
            children(options)
          ) : (
            <Text c='dimmed'>{t('pages.server.minecraft.content.noLoader', {})}</Text>
          )
        }
      </ResourceView>
    </ServerContentContainer>
  );
}
