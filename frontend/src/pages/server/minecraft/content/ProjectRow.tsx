// plugged.host: one search result, installed with one click or a chosen version (docs/features/minecraft.md)
import { faArrowUpRightFromSquare, faListUl } from '@fortawesome/free-solid-svg-icons';
import { useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import installContent from '@/api/server/minecraft/installContent.ts';
import Button from '@/elements/buttons/Button.tsx';
import { ServerCan } from '@/elements/Can.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConditionalTooltip from '@/elements/overlays/ConditionalTooltip.tsx';
import ContextMenu, { ContextMenuToggle } from '@/elements/overlays/ContextMenu.tsx';
import Text from '@/elements/typography/Text.tsx';
import { minecraftProjectSchema } from '@/lib/schemas/server/minecraft.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import InstallVersionModal from './InstallVersionModal.tsx';
import { minecraftSourceLabels } from './labels.ts';
import { useInstalledToast } from './useInstalledToast.ts';

interface Props {
  project: z.infer<typeof minecraftProjectSchema>;
  gameVersion: string | null;
  installed: boolean;
  onInstalled: () => void;
}

export default function ProjectRow({ project, gameVersion, installed, onInstalled }: Props) {
  const { t, language } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const canManage = useServerCan('minecraft.content');
  const installedToast = useInstalledToast();

  const [installing, setInstalling] = useState(false);
  const [versionsOpen, setVersionsOpen] = useState(false);
  const base = 'pages.server.minecraft.content.browse';

  const install = () => {
    setInstalling(true);

    installContent(server.uuid, { source: project.source, project: project.id })
      .then((result) => {
        installedToast(result);
        onInstalled();
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setInstalling(false));
  };

  return (
    <>
      {canManage && project.installable && (
        <InstallVersionModal
          project={project}
          gameVersion={gameVersion}
          opened={versionsOpen}
          onClose={() => setVersionsOpen(false)}
          onInstalled={onInstalled}
        />
      )}

      <ContextMenu
        items={[
          {
            type: 'action',
            icon: faListUl,
            label: t(`${base}.button.chooseVersion`, {}),
            hidden: !project.installable,
            onClick: () => setVersionsOpen(true),
            color: 'gray',
            canAccess: canManage,
          },
          {
            type: 'action',
            icon: faArrowUpRightFromSquare,
            label: t('pages.server.minecraft.content.button.open', { source: minecraftSourceLabels[project.source] }),
            onClick: () => window.open(project.url, '_blank', 'noopener,noreferrer'),
            color: 'gray',
          },
        ]}
      >
        {({ items, openMenu }) => (
          <TableRow
            onContextMenu={(e) => {
              e.preventDefault();
              openMenu(e.clientX, e.clientY);
            }}
          >
            <TableData>
              <Group gap='sm' wrap='nowrap'>
                {project.icon && (
                  <img src={project.icon} alt='' loading='lazy' className='w-10 h-10 rounded-md shrink-0' />
                )}
                <Stack gap={0}>
                  <Group gap='xs'>
                    <Text size='sm' fw={500}>
                      {project.title}
                    </Text>
                    {installed && (
                      <Badge color='green' size='xs'>
                        {t(`${base}.badge.installed`, {})}
                      </Badge>
                    )}
                  </Group>
                  <Text size='xs' c='dimmed' lineClamp={2}>
                    {project.description}
                  </Text>
                </Stack>
              </Group>
            </TableData>

            <TableData>{project.author}</TableData>

            <TableData>{new Intl.NumberFormat(language, { notation: 'compact' }).format(project.downloads)}</TableData>

            <TableData>
              <ServerCan action='minecraft.content'>
                <ConditionalTooltip enabled={!project.installable} label={t(`${base}.externalHint`, {})}>
                  <Button color='gray' size='xs' loading={installing} disabled={!project.installable} onClick={install}>
                    {project.installable ? t(`${base}.button.install`, {}) : t(`${base}.button.external`, {})}
                  </Button>
                </ConditionalTooltip>
              </ServerCan>
            </TableData>

            <ContextMenuToggle items={items} openMenu={openMenu} />
          </TableRow>
        )}
      </ContextMenu>
    </>
  );
}
