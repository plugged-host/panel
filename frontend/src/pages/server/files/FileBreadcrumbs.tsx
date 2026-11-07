import { faArrowUp, faDoorOpen, faRotate } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import classNames from 'classnames';
import { join } from 'pathe';
import { ReactNode } from 'react';
import { createSearchParams, NavLink, useSearchParams } from 'react-router';
import { z } from 'zod';
import { useShallow } from 'zustand/react/shallow';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Button from '@/elements/buttons/Button.tsx';
import Breadcrumbs from '@/elements/data-display/Breadcrumbs.tsx';
import Checkbox from '@/elements/input/Checkbox.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import { pathSegments } from '@/lib/path.ts';
import { serverDirectoryEntrySchema } from '@/lib/schemas/server/files.ts';
import { useDraggedFileMove } from '@/pages/server/files/hooks/useDraggedFileMove.ts';
import { useFileManager } from '@/providers/FileManagerProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

export default function FileBreadcrumbs({
  path,
  inFileEditor,
  entries,
}: {
  path: string;
  inFileEditor?: boolean;
  entries?: z.infer<typeof serverDirectoryEntrySchema>[];
}) {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  const [, setSearchParams] = useSearchParams();
  const { isDropTarget, getDropHandlers } = useDraggedFileMove({ disabled: !!inFileEditor });
  const {
    selectedFilesCount,
    browsingBackup,
    browsingEntries,
    setBrowsingDirectory,
    anyActing,
    doSelectFiles,
    invalidateFilemanager,
  } = useFileManager(
    useShallow((state) => ({
      selectedFilesCount: state.selectedFiles.size,
      browsingBackup: state.browsingBackup,
      browsingEntries: state.browsingEntries,
      setBrowsingDirectory: state.setBrowsingDirectory,
      anyActing: state.actingFiles.size > 0,
      doSelectFiles: state.doSelectFiles,
      invalidateFilemanager: state.invalidateFilemanager,
    })),
  );

  const selectableEntries = entries ?? browsingEntries.data;
  const splittedPath = path.split('/').filter(Boolean);
  const pathItems = pathSegments(path);

  const isBackupPath = path.startsWith('/.backups/');
  const backupUuid = isBackupPath ? (splittedPath[1] ?? '') : null;

  const breadcrumbClassName = (targetDirectory: string) =>
    classNames(
      'text-(--mantine-color-anchor) hover:underline rounded-sm',
      isDropTarget(targetDirectory) && 'bg-(--mantine-color-green-light)',
    );

  const items: ReactNode[] = [
    isBackupPath ? 'backups' : 'home',
    <NavLink
      key='first-segment'
      to={
        isBackupPath
          ? `/server/${server?.uuidShort}/files?${createSearchParams({
              directory: `/.backups/${backupUuid}`,
            })}`
          : `/server/${server?.uuidShort}/files`
      }
      className={breadcrumbClassName(isBackupPath ? `/.backups/${backupUuid}` : '/')}
      {...getDropHandlers(isBackupPath ? `/.backups/${backupUuid}` : '/')}
    >
      {isBackupPath ? (browsingBackup?.name ?? backupUuid) : 'container'}
    </NavLink>,
    ...pathItems.slice(isBackupPath ? 2 : 0).map((item, index) =>
      index === pathItems.length - 1 && inFileEditor ? (
        item.name
      ) : (
        <NavLink
          key={item.path}
          to={`/server/${server?.uuidShort}/files?${createSearchParams({ directory: join('/', item.path) })}`}
          className={breadcrumbClassName(join('/', item.path))}
          onClick={inFileEditor ? undefined : () => setBrowsingDirectory(join('/', item.path))}
          {...getDropHandlers(join('/', item.path))}
        >
          {item.name}
        </NavLink>
      ),
    ),
  ];

  return (
    <div
      id='file-breadcrumbs-inner'
      className='flex flex-col gap-4 sm:gap-0 sm:flex-row sm:items-center sm:justify-between'
    >
      <Breadcrumbs separatorMargin='xs'>
        <Checkbox
          disabled={anyActing}
          checked={!inFileEditor && selectedFilesCount > 0 && selectedFilesCount >= selectableEntries.length}
          indeterminate={selectedFilesCount > 0 && selectedFilesCount < selectableEntries.length}
          className='mr-2'
          classNames={{ input: 'cursor-pointer!' }}
          hidden={inFileEditor}
          onChange={() => {
            if (selectedFilesCount >= selectableEntries.length) {
              doSelectFiles([]);
            } else {
              doSelectFiles(selectableEntries);
            }
          }}
        />
        {items}
      </Breadcrumbs>

      {!inFileEditor && (
        <div className='flex flex-row space-x-2'>
          <NavLink to={`/server/${server?.uuidShort}/files`} hidden={!isBackupPath}>
            <Button variant='light' leftSection={<FontAwesomeIcon icon={faDoorOpen} />}>
              {t('pages.server.files.button.exitBackup', {})}
            </Button>
          </NavLink>
          <span className='flex flex-row gap-2'>
            <Tooltip label={t('pages.server.files.quickAction.parentDirectory', {})}>
              <ActionIcon
                variant='light'
                color='gray'
                size='input-sm'
                disabled={path === '/' || path === ''}
                aria-label={t('pages.server.files.quickAction.parentDirectory', {})}
                onClick={() => {
                  const parent = join(path, '..');
                  setBrowsingDirectory(parent);
                  setSearchParams({ directory: parent });
                }}
              >
                <FontAwesomeIcon icon={faArrowUp} />
              </ActionIcon>
            </Tooltip>
            <Tooltip label={t('pages.server.files.tree.reload', {})}>
              <ActionIcon
                variant='light'
                color='cyan'
                size='input-sm'
                aria-label={t('pages.server.files.tree.reload', {})}
                onClick={() => invalidateFilemanager()}
              >
                <FontAwesomeIcon icon={faRotate} />
              </ActionIcon>
            </Tooltip>
          </span>
        </div>
      )}
    </div>
  );
}
