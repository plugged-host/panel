// plugged.host: the Plugins / Mods & Modpacks page: what is installed, then the catalog (docs/features/minecraft.md)
import { faArrowsRotate, faPuzzlePiece } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import getBedrock from '@/api/server/minecraft/getBedrock.ts';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import EmptyState from '@/elements/feedback/EmptyState.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import BedrockModal from './BedrockModal.tsx';
import BrowseTable from './BrowseTable.tsx';
import InstalledSection from './InstalledSection.tsx';
import ModpackInstallCallout from './ModpackInstallCallout.tsx';
import { useContentOptions } from './useContentOptions.ts';

// the loaders Geyser has a build for (shared/src/minecraft/bedrock.rs)
const BEDROCK_LOADERS = ['paper', 'fabric', 'neoforge'];

const base = 'pages.server.minecraft.content';

/** "Add Bedrock support", or "Update Geyser" once it is there; hidden where Geyser has no build. */
function BedrockButton() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  const [open, setOpen] = useState(false);

  const bedrock = useQuery({
    queryKey: queryKeys.server(server.uuid).minecraft.bedrock(),
    queryFn: () => getBedrock(server.uuid),
    retry: false,
  });
  if (!bedrock.data?.available) return null;

  const label = bedrock.data.installed
    ? t('pages.server.minecraft.bedrock.button.update', {})
    : t('pages.server.minecraft.bedrock.button.add', {});

  return (
    <>
      <BedrockModal status={bedrock.data} label={label} opened={open} onClose={() => setOpen(false)} />
      <Button variant='default' onClick={() => setOpen(true)}>
        {label}
      </Button>
    </>
  );
}

export default function ServerMinecraftContent() {
  const { t } = useTranslations();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const canBedrock = useServerCan('minecraft.bedrock');
  const options = useContentOptions();

  const loader = options.data?.software.loader ?? null;
  // the Pelican panel names the page after what the loader takes
  const plugins = options.data?.kinds.includes('plugin') ?? false;

  const rescan = () => {
    const keys = queryKeys.server(server.uuid).minecraft;
    queryClient.invalidateQueries({ queryKey: keys.installed() });
    queryClient.invalidateQueries({ queryKey: keys.content() });
    queryClient.invalidateQueries({ queryKey: keys.bedrock() });
  };

  return (
    <ServerContentContainer
      title={
        !options.data ? t(`${base}.title`, {}) : plugins ? t(`${base}.titlePlugins`, {}) : t(`${base}.titleMods`, {})
      }
      subtitle={!options.data ? undefined : plugins ? t(`${base}.subtitlePlugins`, {}) : t(`${base}.subtitleMods`, {})}
      contentRight={
        <>
          {canBedrock && loader && BEDROCK_LOADERS.includes(loader) && <BedrockButton />}
          <Tooltip label={t(`${base}.button.rescan`, {})}>
            <ActionIcon
              size='input-sm'
              variant='subtle'
              color='gray'
              aria-label={t(`${base}.button.rescan`, {})}
              onClick={rescan}
            >
              <FontAwesomeIcon icon={faArrowsRotate} />
            </ActionIcon>
          </Tooltip>
        </>
      }
    >
      <ResourceView resource={options}>
        {(data) =>
          !data.software.loader && !data.modpacks ? (
            <EmptyState
              flush
              icon={faPuzzlePiece}
              title={t(`${base}.title`, {})}
              description={t(`${base}.noLoader`, {})}
            />
          ) : (
            <div className='flex flex-col gap-8'>
              {data.modpacks && <ModpackInstallCallout />}
              {data.software.loader && <InstalledSection options={data} />}
              <BrowseTable options={data} />
            </div>
          )
        }
      </ResourceView>
    </ServerContentContainer>
  );
}
