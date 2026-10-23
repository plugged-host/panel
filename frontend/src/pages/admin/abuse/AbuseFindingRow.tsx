// plugged.host: Admin › Abuse: one finding.
import { faCheck, faEye, faXmark } from '@fortawesome/free-solid-svg-icons';
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import updateAbuseFinding from '@/api/admin/abuse/updateAbuseFinding.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Badge from '@/elements/data-display/Badge.tsx';
import { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import TableLink from '@/elements/data-display/TableLink.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ContextMenu, { ContextMenuToggle } from '@/elements/overlays/ContextMenu.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Code from '@/elements/typography/Code.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { AdminAbuseFinding } from '@/lib/schemas/admin/abuse.ts';
import { useAdminCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import {
  findingStatusColorMapping,
  findingStatusLabelMapping,
  severityColorMapping,
  severityLabelMapping,
} from './abuseLabels.ts';
import AbuseFindingModal from './modals/AbuseFindingModal.tsx';

export default function AbuseFindingRow({ finding }: { finding: AdminAbuseFinding }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const canManage = useAdminCan('abuse.manage');
  const [openModal, setOpenModal] = useState<'view' | 'dismiss' | null>(null);

  const close = async (status: 'dismissed' | 'resolved') => {
    await updateAbuseFinding(finding.uuid, status)
      .then(() => {
        setOpenModal(null);
        queryClient.invalidateQueries({ queryKey: queryKeys.admin.abuse.findings() });
        addToast(
          status === 'dismissed'
            ? t('pages.admin.abuse.page.toast.dismissed', {})
            : t('pages.admin.abuse.page.toast.resolved', {}),
          'success',
        );
      })
      .catch((msg) => {
        addToast(httpErrorToHuman(msg), 'error');
      });
  };

  // A link opens its page, not the finding.
  const target = finding.server ? (
    <TableLink to={`/admin/servers/${finding.server.uuid}`} onClick={(e) => e.stopPropagation()}>
      <Code>{finding.server.name}</Code>
    </TableLink>
  ) : finding.node ? (
    <TableLink to={`/admin/nodes/${finding.node.uuid}`} onClick={(e) => e.stopPropagation()}>
      <Code>{finding.node.name}</Code>
    </TableLink>
  ) : (
    <Text size='sm' c='dimmed'>
      {t('common.na', {})}
    </Text>
  );

  return (
    <>
      <AbuseFindingModal finding={finding} opened={openModal === 'view'} onClose={() => setOpenModal(null)} />
      <ConfirmationModal
        opened={openModal === 'dismiss'}
        onClose={() => setOpenModal(null)}
        title={t('pages.admin.abuse.page.modal.dismiss.title', {})}
        confirm={t('pages.admin.abuse.page.button.dismiss', {})}
        onConfirmed={() => close('dismissed')}
      >
        {t('pages.admin.abuse.page.modal.dismiss.content', {}).md()}
      </ConfirmationModal>

      <ContextMenu
        items={[
          {
            type: 'action',
            icon: faEye,
            label: t('common.button.details', {}),
            onClick: () => setOpenModal('view'),
          },
          {
            type: 'action',
            icon: faCheck,
            label: t('pages.admin.abuse.page.button.resolve', {}),
            onClick: () => close('resolved'),
            canAccess: canManage && (finding.status === 'open' || finding.status === 'actioned'),
          },
          {
            type: 'action',
            icon: faXmark,
            label: t('pages.admin.abuse.page.button.dismiss', {}),
            onClick: () => setOpenModal('dismiss'),
            color: 'red',
            canAccess: canManage && finding.status !== 'dismissed',
          },
        ]}
      >
        {({ items, openMenu }) => (
          <TableRow
            onClick={() => setOpenModal('view')}
            onContextMenu={(e) => {
              e.preventDefault();
              openMenu(e.clientX, e.clientY);
            }}
          >
            <TableData>{target}</TableData>
            <TableData>{finding.categoryLabel}</TableData>
            <TableData>
              <Badge color={severityColorMapping[finding.severity]} variant='light'>
                {severityLabelMapping[finding.severity]()}
              </Badge>
            </TableData>
            <TableData>
              <Text size='sm' lineClamp={2}>
                {finding.summary}
              </Text>
            </TableData>
            <TableData>
              <Badge color={findingStatusColorMapping[finding.status]} variant='light'>
                {findingStatusLabelMapping[finding.status]()}
              </Badge>
            </TableData>
            <TableData>
              <FormattedTimestamp timestamp={finding.lastSeen} />
            </TableData>

            <ContextMenuToggle items={items} openMenu={openMenu} />
          </TableRow>
        )}
      </ContextMenu>
    </>
  );
}
