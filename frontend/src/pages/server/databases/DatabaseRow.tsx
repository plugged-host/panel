import {
  faEye,
  faLock,
  faLockOpen,
  faPencil,
  faRefresh,
  faTableList,
  faTrash,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { forwardRef, useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import getDatabaseSize from '@/api/server/databases/getDatabaseSize.ts';
import CopyOnClick from '@/elements/CopyOnClick.tsx';
import { TableData, TableRow, TableSelectionCell } from '@/elements/data-display/Table.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import ContextMenu, { ContextMenuToggle } from '@/elements/overlays/ContextMenu.tsx';
import Code from '@/elements/typography/Code.tsx';
import RedactedText from '@/elements/typography/RedactedText.tsx';
import { databaseTypeLabelMapping } from '@/lib/enums.ts';
import { bytesToString } from '@/lib/format/size.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverDatabaseSchema } from '@/lib/schemas/server/databases.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import DatabaseDeleteModal from './modals/DatabaseDeleteModal.tsx';
import DatabaseDetailsModal from './modals/DatabaseDetailsModal.tsx';
import DatabaseEditModal from './modals/DatabaseEditModal.tsx';
import DatabaseRecreateModal from './modals/DatabaseRecreateModal.tsx';

interface DatabaseRowProps {
  database: z.infer<typeof serverDatabaseSchema>;
  isSelected?: boolean;
  onSelectionChange?: (selected: boolean, range?: boolean) => void;
}

const DatabaseRow = forwardRef<HTMLTableRowElement, DatabaseRowProps>(function DatabaseRow(
  { database, isSelected = false, onSelectionChange },
  ref,
) {
  const { t } = useTranslations();
  const navigate = useNavigate();
  const [openModal, setOpenModal] = useState<'edit' | 'details' | 'recreate' | 'delete' | null>(null);
  const server = useServerStore((state) => state.server);
  const host = `${database.host}:${database.port}`;

  const {
    data: size,
    loading: sizeLoading,
    refetch: refetchSize,
  } = useResource({
    queryKey: queryKeys.server(server.uuid).databases.size(database.uuid),
    queryFn: () => getDatabaseSize(server.uuid, database.uuid),
  });

  return (
    <>
      <DatabaseEditModal database={database} opened={openModal === 'edit'} onClose={() => setOpenModal(null)} />
      <DatabaseDetailsModal database={database} opened={openModal === 'details'} onClose={() => setOpenModal(null)} />
      <DatabaseRecreateModal
        database={database}
        opened={openModal === 'recreate'}
        onClose={() => setOpenModal(null)}
        onRecreated={refetchSize}
      />
      <DatabaseDeleteModal database={database} opened={openModal === 'delete'} onClose={() => setOpenModal(null)} />

      <ContextMenu
        items={[
          {
            type: 'action',
            icon: faPencil,
            label: t('common.button.edit', {}),
            onClick: () => setOpenModal('edit'),
            color: 'gray',
            canAccess: useServerCan('databases.update'),
          },
          {
            type: 'action',
            icon: faEye,
            label: t('common.button.details', {}),
            onClick: () => setOpenModal('details'),
            color: 'gray',
            canAccess: useServerCan('databases.read'),
          },
          {
            type: 'action',
            icon: faTableList,
            label: t('pages.server.databases.explorer.button.open', {}),
            hidden: database.type === 'mongodb',
            onClick: () => navigate(`/server/${server.uuidShort}/databases/${database.uuid}/explore`),
            color: 'gray',
            canAccess: useServerCan(['databases.read', 'databases.query'], false),
          },
          {
            type: 'action',
            icon: faRefresh,
            label: t('common.button.recreate', {}),
            disabled: database.isLocked,
            onClick: () => setOpenModal('recreate'),
            color: 'red',
            canAccess: useServerCan('databases.recreate'),
          },
          {
            type: 'action',
            icon: faTrash,
            label: t('common.button.delete', {}),
            disabled: database.isLocked,
            onClick: () => setOpenModal('delete'),
            color: 'red',
            canAccess: useServerCan('databases.delete'),
          },
        ]}
        registry={window.extensionContext.extensionRegistry.pages.server.databases.databaseContextMenu}
        registryProps={{ database }}
      >
        {({ items, openMenu }) => (
          <TableRow
            ref={ref}
            bg={isSelected ? 'var(--mantine-color-blue-light)' : undefined}
            onContextMenu={(e) => {
              e.preventDefault();
              openMenu(e.clientX, e.clientY);
            }}
          >
            {onSelectionChange !== undefined && (
              <TableSelectionCell id={database.uuid} checked={isSelected} onChange={onSelectionChange} />
            )}

            <TableData>{database.name}</TableData>

            <TableData>{databaseTypeLabelMapping[database.type]}</TableData>

            <TableData>
              <CopyOnClick content={host}>
                <Code>
                  <RedactedText value={host} />
                </Code>
              </CopyOnClick>
            </TableData>

            <TableData>{database.username}</TableData>

            <TableData>{sizeLoading ? <Spinner size={16} /> : bytesToString(size ?? 0)}</TableData>

            <TableData>
              {database.isLocked ? (
                <FontAwesomeIcon className='text-(--mantine-color-green-text)' icon={faLock} />
              ) : (
                <FontAwesomeIcon className='text-(--mantine-color-red-text)' icon={faLockOpen} />
              )}
            </TableData>

            <ContextMenuToggle items={items} openMenu={openMenu} />
          </TableRow>
        )}
      </ContextMenu>
    </>
  );
});

export default DatabaseRow;
