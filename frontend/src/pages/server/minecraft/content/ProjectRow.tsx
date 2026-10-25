// plugged.host: one catalog row, installed with one click or a chosen version (docs/features/minecraft.md)
import { faArrowUpRightFromSquare } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import installContent from '@/api/server/minecraft/installContent.ts';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import ConditionalTooltip from '@/elements/overlays/ConditionalTooltip.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import { minecraftProjectSchema } from '@/lib/schemas/server/minecraft.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import InstallVersionModal from './InstallVersionModal.tsx';
import { minecraftSourceLabels } from './labels.ts';
import ModpackModal from './ModpackModal.tsx';
import { useInstalledToast } from './useInstalledToast.ts';

export interface ProjectColumns {
  author: boolean;
  categories: boolean;
}

interface Props {
  project: z.infer<typeof minecraftProjectSchema>;
  columns: ProjectColumns;
  gameVersion: string | null;
  installed: boolean;
  // a modpack cannot be installed now (the server runs, or a pack is being installed)
  modpackBlocked: 'running' | 'busy' | null;
  onInstalled: () => void;
  onModpackStarted: () => void;
}

export default function ProjectRow({
  project,
  columns,
  gameVersion,
  installed,
  modpackBlocked,
  onInstalled,
  onModpackStarted,
}: Props) {
  const { t, language } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const canManage = useServerCan('minecraft.content');
  const canModpack = useServerCan(['minecraft.content', 'settings.software'], false);
  const installedToast = useInstalledToast();

  const [installing, setInstalling] = useState(false);
  const [open, setOpen] = useState(false);
  const base = 'pages.server.minecraft.content.browse';
  const modpack = project.kind === 'modpack';

  const quickInstall = () => {
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
    <TableRow>
      <TableData>
        {project.icon ? (
          <img src={project.icon} alt='' loading='lazy' className='theme-catalog-icon' />
        ) : (
          <span className='theme-catalog-icon' />
        )}
      </TableData>

      <TableData className='theme-catalog-project'>
        <span className='theme-catalog-title'>{project.title}</span>
        {project.description && <span className='theme-catalog-description'>{project.description}</span>}
      </TableData>

      {columns.author && <TableData>{project.author}</TableData>}

      <TableData>{project.downloads.toLocaleString(language)}</TableData>

      {columns.categories && (
        <TableData>
          <div className='theme-catalog-categories'>
            {project.categories.slice(0, 3).map((category) => (
              <Badge key={category} color='gray' size='sm'>
                {category}
              </Badge>
            ))}
            {project.categories.length > 3 && (
              <span className='theme-catalog-more'>
                {t(`${base}.table.more`, { count: project.categories.length - 3 })}
              </span>
            )}
          </div>
        </TableData>
      )}

      <TableData>
        {installed ? (
          <Badge color='green' size='sm'>
            {t(`${base}.badge.installed`, {})}
          </Badge>
        ) : !project.installable ? (
          <Tooltip label={t(`${base}.externalHint`, {})}>
            <Badge color='gray' size='sm'>
              {t(`${base}.badge.external`, {})}
            </Badge>
          </Tooltip>
        ) : null}
      </TableData>

      <TableData>
        <div className='theme-catalog-actions'>
          {modpack
            ? canModpack && (
                <>
                  {project.installable && (
                    <ModpackModal
                      project={project}
                      opened={open}
                      onClose={() => setOpen(false)}
                      onStarted={onModpackStarted}
                    />
                  )}
                  <ConditionalTooltip
                    enabled={modpackBlocked === 'running'}
                    label={t('pages.server.minecraft.modpacks.running', {})}
                  >
                    <button
                      type='button'
                      className='theme-link-action'
                      data-accent
                      disabled={!project.installable || modpackBlocked !== null}
                      onClick={() => setOpen(true)}
                    >
                      {t('common.button.install', {})}
                    </button>
                  </ConditionalTooltip>
                </>
              )
            : canManage && (
                <>
                  {project.installable && (
                    <InstallVersionModal
                      project={project}
                      gameVersion={gameVersion}
                      opened={open}
                      onClose={() => setOpen(false)}
                      onInstalled={onInstalled}
                    />
                  )}
                  <button
                    type='button'
                    className='theme-link-action'
                    data-accent
                    disabled={!project.installable || installing}
                    onClick={quickInstall}
                  >
                    {t(`${base}.button.quickInstall`, {})}
                  </button>
                  <button
                    type='button'
                    className='theme-link-action'
                    disabled={!project.installable}
                    onClick={() => setOpen(true)}
                  >
                    {t(`${base}.button.versions`, {})}
                  </button>
                </>
              )}
          <Tooltip
            label={t('pages.server.minecraft.content.button.view', { source: minecraftSourceLabels[project.source] })}
          >
            <ActionIcon
              size='input-sm'
              variant='subtle'
              color='gray'
              aria-label={t('pages.server.minecraft.content.button.view', {
                source: minecraftSourceLabels[project.source],
              })}
              onClick={() => window.open(project.url, '_blank', 'noopener,noreferrer')}
            >
              <FontAwesomeIcon icon={faArrowUpRightFromSquare} />
            </ActionIcon>
          </Tooltip>
        </div>
      </TableData>
    </TableRow>
  );
}
