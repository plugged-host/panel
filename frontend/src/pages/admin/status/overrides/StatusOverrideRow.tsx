// plugged.host: Admin › Status › Overrides: one override.
import { faTrash } from '@fortawesome/free-solid-svg-icons';
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import deleteStatusOverride from '@/api/admin/status/deleteStatusOverride.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Badge from '@/elements/data-display/Badge.tsx';
import { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ContextMenu, { ContextMenuToggle } from '@/elements/overlays/ContextMenu.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import BooleanText from '@/elements/typography/BooleanText.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { AdminStatusComponent, AdminStatusOverride } from '@/lib/schemas/admin/status.ts';
import { useAdminCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { componentStatusColorMapping, componentStatusLabelMapping } from '../statusLabels.ts';

export default function StatusOverrideRow({
  override,
  components,
}: {
  override: AdminStatusOverride;
  components: AdminStatusComponent[];
}) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const canManage = useAdminCan('status.manage');
  const [openModal, setOpenModal] = useState<'delete' | null>(null);

  const doDelete = async () => {
    await deleteStatusOverride(override.uuid)
      .then(() => {
        setOpenModal(null);
        queryClient.invalidateQueries({ queryKey: queryKeys.admin.status.overrides() });
        addToast(t('pages.admin.status.tabs.overrides.page.toast.deleted', {}), 'success');
      })
      .catch((msg) => {
        addToast(httpErrorToHuman(msg), 'error');
      });
  };

  const component = override.componentUuid
    ? (components.find((component) => component.uuid === override.componentUuid)?.label ?? override.componentUuid)
    : t('pages.admin.status.label.allComponents', {});
  const na = (
    <Text size='sm' c='dimmed'>
      {t('common.na', {})}
    </Text>
  );

  return (
    <>
      <ConfirmationModal
        opened={openModal === 'delete'}
        onClose={() => setOpenModal(null)}
        title={t('pages.admin.status.tabs.overrides.page.modal.delete.title', {})}
        confirm={t('common.button.delete', {})}
        onConfirmed={doDelete}
      >
        {t('pages.admin.status.tabs.overrides.page.modal.delete.content', { component }).md()}
      </ConfirmationModal>

      <ContextMenu
        items={[
          {
            type: 'action',
            icon: faTrash,
            label: t('common.button.delete', {}),
            onClick: () => setOpenModal('delete'),
            color: 'red',
            canAccess: canManage,
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
            <TableData>{component}</TableData>
            <TableData>
              <Badge color={componentStatusColorMapping[override.status]} variant='light'>
                {componentStatusLabelMapping[override.status]()}
              </Badge>
            </TableData>
            <TableData>{override.note ?? na}</TableData>
            <TableData>{override.starts ? <FormattedTimestamp timestamp={override.starts} /> : na}</TableData>
            <TableData>{override.ends ? <FormattedTimestamp timestamp={override.ends} /> : na}</TableData>
            <TableData>
              <BooleanText value={override.active} />
            </TableData>

            <ContextMenuToggle items={items} openMenu={openMenu} />
          </TableRow>
        )}
      </ContextMenu>
    </>
  );
}
