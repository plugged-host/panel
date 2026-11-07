import {
  faDownload,
  faEllipsis,
  faFileCirclePlus,
  faFileUpload,
  faFolderOpen,
  faFolderPlus,
  faLink,
  faMagnifyingGlassChart,
  faSearch,
  faTrashCan,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useEffect, useRef, useState } from 'react';
import { createSearchParams, useNavigate } from 'react-router';
import { useShallow } from 'zustand/react/shallow';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Button from '@/elements/buttons/Button.tsx';
import { ServerCan } from '@/elements/Can.tsx';
import ExtensionSlot from '@/elements/ExtensionSlot.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Group from '@/elements/layout/Group.tsx';
import ContextMenu from '@/elements/overlays/ContextMenu.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useFileManager } from '@/providers/FileManagerProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import FileConnectButton from '../FileConnectButton.tsx';

export default function FileToolbar({
  onCreateFile,
  treeView,
  filter,
  onFilterChange,
}: {
  onCreateFile?: () => void;
  treeView: boolean;
  filter: string;
  onFilterChange: (value: string) => void;
}) {
  const { t } = useTranslations();
  const navigate = useNavigate();
  const canCreate = useServerCan('files.create');
  const canRead = useServerCan('files.read');
  const [searchOpen, setSearchOpen] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);
  const server = useServerStore((state) => state.server);
  const { fileInputRef, folderInputRef, browsingDirectory, browsingWritableDirectory, doOpenModal } = useFileManager(
    useShallow((state) => ({
      fileInputRef: state.fileInputRef,
      folderInputRef: state.folderInputRef,
      browsingDirectory: state.browsingDirectory,
      browsingWritableDirectory: state.browsingWritableDirectory,
      doOpenModal: state.doOpenModal,
    })),
  );

  useEffect(() => {
    if (searchOpen) searchRef.current?.focus();
  }, [searchOpen]);

  const newFile = () => {
    if (onCreateFile) {
      onCreateFile();
      return;
    }
    navigate(`/server/${server.uuidShort}/files/new?${createSearchParams({ directory: browsingDirectory })}`);
  };

  const moreItems = [
    ...(canCreate && !treeView && browsingWritableDirectory
      ? [
          {
            type: 'action' as const,
            icon: faDownload,
            label: t('pages.server.files.button.fileFromPull', {}),
            onClick: () => doOpenModal('pullFile'),
            color: 'gray' as const,
          },
          {
            type: 'action' as const,
            icon: faLink,
            label: t('pages.server.files.button.symlink', {}),
            onClick: () => doOpenModal('nameSymlink', []),
            color: 'gray' as const,
          },
        ]
      : []),
    ...(canRead
      ? [
          {
            type: 'action' as const,
            icon: faSearch,
            label: t('pages.server.files.button.search', {}),
            onClick: () => doOpenModal('search'),
            color: 'gray' as const,
          },
          {
            type: 'action' as const,
            icon: faMagnifyingGlassChart,
            label: t('pages.server.files.tooltip.largestDirectories', {}),
            onClick: () => doOpenModal('largestDirectories'),
            color: 'gray' as const,
          },
        ]
      : []),
  ];

  return (
    <Group className='min-w-0 flex-1'>
      <ExtensionSlot
        components={window.extensionContext.extensionRegistry.pages.server.files.fileToolbar.prependedComponents}
        name='files-fileToolbar-prepended'
      />

      {!treeView && (
        <Group gap='xs' className='min-w-0'>
          <Tooltip label={t('pages.server.files.searchPlaceholder', {})}>
            <ActionIcon
              variant={searchOpen ? 'light' : 'subtle'}
              color='gray'
              aria-label={t('pages.server.files.searchPlaceholder', {})}
              onClick={() => {
                if (searchOpen) onFilterChange('');
                setSearchOpen((open) => !open);
              }}
            >
              <FontAwesomeIcon icon={faSearch} />
            </ActionIcon>
          </Tooltip>
          <div
            className={`overflow-hidden transition-all duration-300 ease-out ${
              searchOpen ? 'w-full opacity-100 sm:w-[clamp(16rem,40vw,32rem)]' : 'w-0 opacity-0'
            }`}
          >
            <TextInput
              ref={searchRef}
              placeholder={t('pages.server.files.searchPlaceholder', {})}
              aria-label={t('pages.server.files.searchPlaceholder', {})}
              value={filter}
              onChange={(event) => onFilterChange(event.currentTarget.value)}
              className='w-full'
            />
          </div>
        </Group>
      )}

      <FileConnectButton iconOnly />

      {!treeView && browsingWritableDirectory && (
        <ServerCan action='files.create'>
          <Tooltip label={t('pages.server.files.button.newFolder', {})}>
            <ActionIcon
              variant='subtle'
              color='gray'
              aria-label={t('pages.server.files.button.newFolder', {})}
              onClick={() => doOpenModal('nameDirectory')}
            >
              <FontAwesomeIcon icon={faFolderPlus} />
            </ActionIcon>
          </Tooltip>
        </ServerCan>
      )}

      {browsingWritableDirectory && (
        <ServerCan action='files.create'>
          <Tooltip label={t('pages.server.files.button.newFile', {})}>
            <ActionIcon
              variant='subtle'
              color='gray'
              aria-label={t('pages.server.files.button.newFile', {})}
              onClick={newFile}
            >
              <FontAwesomeIcon icon={faFileCirclePlus} />
            </ActionIcon>
          </Tooltip>
        </ServerCan>
      )}

      {!treeView && browsingWritableDirectory && (
        <ServerCan action='files.create'>
          <Tooltip label={t('pages.server.files.button.directoryFromUpload', {})}>
            <Button
              variant='default'
              className='theme-file-header-action'
              data-collapsed={searchOpen}
              aria-label={t('pages.server.files.button.directoryFromUpload', {})}
              leftSection={<FontAwesomeIcon icon={faFolderOpen} />}
              onClick={() => folderInputRef.current?.click()}
            >
              <span className='theme-file-header-label'>{t('pages.server.files.button.directoryFromUpload', {})}</span>
            </Button>
          </Tooltip>
        </ServerCan>
      )}

      {!treeView && browsingWritableDirectory && (
        <ServerCan action='files.create'>
          <Tooltip label={t('pages.server.files.button.upload', {})}>
            <Button
              className='theme-file-header-action'
              data-collapsed={searchOpen}
              aria-label={t('pages.server.files.button.upload', {})}
              leftSection={<FontAwesomeIcon icon={faFileUpload} />}
              onClick={() => fileInputRef.current?.click()}
            >
              <span className='theme-file-header-label'>{t('pages.server.files.button.upload', {})}</span>
            </Button>
          </Tooltip>
        </ServerCan>
      )}

      <ServerCan action='files.read'>
        <Tooltip label={t('pages.server.files.button.trash', {})}>
          <ActionIcon
            variant='subtle'
            color='gray'
            aria-label={t('pages.server.files.button.trash', {})}
            onClick={() => navigate(`/server/${server.uuidShort}/files/trash`)}
          >
            <FontAwesomeIcon icon={faTrashCan} />
          </ActionIcon>
        </Tooltip>
      </ServerCan>

      <Tooltip label={t('pages.server.files.button.more', {})}>
        <ContextMenu items={moreItems}>
          {({ openMenu }) => (
            <ActionIcon
              variant='subtle'
              color='gray'
              aria-label={t('pages.server.files.button.more', {})}
              onClick={(event) => {
                event.stopPropagation();
                const rect = event.currentTarget.getBoundingClientRect();
                openMenu(rect.left, rect.bottom);
              }}
            >
              <FontAwesomeIcon icon={faEllipsis} />
            </ActionIcon>
          )}
        </ContextMenu>
      </Tooltip>

      <ExtensionSlot
        components={window.extensionContext.extensionRegistry.pages.server.files.fileToolbar.appendedComponents}
        name='files-fileToolbar-appended'
      />
    </Group>
  );
}
