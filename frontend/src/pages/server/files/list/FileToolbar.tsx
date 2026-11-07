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
            icon: faFolderOpen,
            label: t('pages.server.files.button.directoryFromUpload', {}),
            onClick: () => folderInputRef.current?.click(),
            color: 'gray' as const,
          },
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
          {
            type: 'action' as const,
            icon: faTrashCan,
            label: t('pages.server.files.button.trash', {}),
            onClick: () => navigate(`/server/${server.uuidShort}/files/trash`),
            color: 'gray' as const,
          },
        ]
      : []),
  ];

  return (
    <Group>
      <ExtensionSlot
        components={window.extensionContext.extensionRegistry.pages.server.files.fileToolbar.prependedComponents}
        name='files-fileToolbar-prepended'
      />

      {!treeView && (
        <TextInput
          leftSection={<FontAwesomeIcon icon={faSearch} />}
          placeholder={t('pages.server.files.searchPlaceholder', {})}
          aria-label={t('pages.server.files.searchPlaceholder', {})}
          value={filter}
          onChange={(event) => onFilterChange(event.currentTarget.value)}
          className='w-full sm:w-60'
        />
      )}

      <FileConnectButton />

      {!treeView && browsingWritableDirectory && (
        <ServerCan action='files.create'>
          <Button
            variant='default'
            leftSection={<FontAwesomeIcon icon={faFolderPlus} />}
            onClick={() => doOpenModal('nameDirectory')}
          >
            {t('pages.server.files.button.newFolder', {})}
          </Button>
        </ServerCan>
      )}

      {browsingWritableDirectory && (
        <ServerCan action='files.create'>
          <Button variant='default' leftSection={<FontAwesomeIcon icon={faFileCirclePlus} />} onClick={newFile}>
            {t('pages.server.files.button.newFile', {})}
          </Button>
        </ServerCan>
      )}

      {!treeView && browsingWritableDirectory && (
        <ServerCan action='files.create'>
          <Button leftSection={<FontAwesomeIcon icon={faFileUpload} />} onClick={() => fileInputRef.current?.click()}>
            {t('pages.server.files.button.upload', {})}
          </Button>
        </ServerCan>
      )}

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
