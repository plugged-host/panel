// plugged.host: Admin › Status › Incidents: one incident, its newest update below the title.
import { faPenToSquare, faTrash } from '@fortawesome/free-solid-svg-icons';
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import deleteStatusIncident from '@/api/admin/status/deleteStatusIncident.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Badge from '@/elements/data-display/Badge.tsx';
import { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ContextMenu, { ContextMenuToggle } from '@/elements/overlays/ContextMenu.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { AdminStatusComponent, AdminStatusIncident } from '@/lib/schemas/admin/status.ts';
import { useAdminCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { incidentImpactColorMapping, incidentImpactLabelMapping, incidentStatusLabelMapping } from '../statusLabels.ts';
import IncidentUpdateModal from './modals/IncidentUpdateModal.tsx';

export default function StatusIncidentRow({
  incident,
  components,
}: {
  incident: AdminStatusIncident;
  components: AdminStatusComponent[];
}) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const canManage = useAdminCan('status.manage');

  const [openModal, setOpenModal] = useState<'update' | 'delete' | null>(null);

  const doDelete = async () => {
    await deleteStatusIncident(incident.uuid)
      .then(() => {
        setOpenModal(null);
        queryClient.invalidateQueries({ queryKey: queryKeys.admin.status.incidents() });
        addToast(t('pages.admin.status.tabs.incidents.page.toast.deleted', {}), 'success');
      })
      .catch((msg) => {
        addToast(httpErrorToHuman(msg), 'error');
      });
  };

  const labels = incident.components.map(
    (uuid) => components.find((component) => component.uuid === uuid)?.label ?? uuid,
  );
  const newest = incident.updates[0];

  return (
    <>
      <IncidentUpdateModal incident={incident} opened={openModal === 'update'} onClose={() => setOpenModal(null)} />
      <ConfirmationModal
        opened={openModal === 'delete'}
        onClose={() => setOpenModal(null)}
        title={t('pages.admin.status.tabs.incidents.page.modal.delete.title', {})}
        confirm={t('common.button.delete', {})}
        onConfirmed={doDelete}
      >
        {t('pages.admin.status.tabs.incidents.page.modal.delete.content', { title: incident.title }).md()}
      </ConfirmationModal>

      <ContextMenu
        items={[
          {
            type: 'action',
            icon: faPenToSquare,
            label: t('pages.admin.status.tabs.incidents.page.button.postUpdate', {}),
            onClick: () => setOpenModal('update'),
            canAccess: canManage,
          },
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
            <TableData>
              <Text size='sm'>{incident.title}</Text>
              {newest && (
                <Text size='xs' c='dimmed' lineClamp={2}>
                  {newest.message}
                </Text>
              )}
            </TableData>
            <TableData>
              <Badge color={incidentImpactColorMapping[incident.impact]} variant='light'>
                {incidentImpactLabelMapping[incident.impact]()}
              </Badge>
            </TableData>
            <TableData>{incidentStatusLabelMapping[incident.status]()}</TableData>
            <TableData>
              {labels.length === 0 ? t('pages.admin.status.label.allComponents', {}) : labels.join(', ')}
            </TableData>
            <TableData>
              <FormattedTimestamp timestamp={incident.started} />
            </TableData>
            <TableData>
              {incident.resolved ? (
                <FormattedTimestamp timestamp={incident.resolved} />
              ) : (
                <Text size='sm' c='dimmed'>
                  {t('common.na', {})}
                </Text>
              )}
            </TableData>

            <ContextMenuToggle items={items} openMenu={openMenu} />
          </TableRow>
        )}
      </ContextMenu>
    </>
  );
}
