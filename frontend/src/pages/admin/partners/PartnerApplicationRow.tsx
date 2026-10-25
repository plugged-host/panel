// plugged.host: Admin › Partners: one application or partnership, with what an admin can do to it.
import { faArrowsRotate, faCheck, faCoins, faEye, faPause, faPlay, faXmark } from '@fortawesome/free-solid-svg-icons';
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import updatePartnerApplication, { PartnerAction } from '@/api/admin/partners/updatePartnerApplication.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Badge from '@/elements/data-display/Badge.tsx';
import { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import TableLink from '@/elements/data-display/TableLink.tsx';
import ContextMenu, { ContextMenuToggle } from '@/elements/overlays/ContextMenu.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { AdminPartnerApplication } from '@/lib/schemas/admin/partners.ts';
import { useAdminCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import PartnerApplicationModal from './modals/PartnerApplicationModal.tsx';
import PartnerCreditsModal from './modals/PartnerCreditsModal.tsx';
import PartnerReasonModal from './modals/PartnerReasonModal.tsx';
import { partnerStatusColorMapping, partnerStatusLabelMapping } from './partnerLabels.ts';

type OpenModal = 'view' | 'approve' | 'credits' | 'reject' | 'revoke' | 'pause' | null;

const PARTNERSHIPS = ['approved', 'paused', 'suspended'];

export default function PartnerApplicationRow({ application }: { application: AdminPartnerApplication }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const canManage = useAdminCan('partners.manage');
  const [openModal, setOpenModal] = useState<OpenModal>(null);
  const [checking, setChecking] = useState(false);

  const partnership = PARTNERSHIPS.includes(application.status);
  const failing = application.status === 'approved' && application.failingSince !== null;

  const run = async (action: PartnerAction, toast: string) => {
    await updatePartnerApplication(application.uuid, action).then(() => {
      setOpenModal(null);
      queryClient.invalidateQueries({ queryKey: queryKeys.admin.partners.all() });
      addToast(toast, 'success');
    });
  };

  const recheck = () => {
    setChecking(true);
    run({ action: 'recheck' }, t('pages.admin.partners.page.toast.rechecked', {}))
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setChecking(false));
  };

  const neo = application.assessment ? (
    <Badge color={application.assessment.accept ? 'green' : 'red'} variant='light'>
      {application.assessment.accept
        ? t('pages.admin.partners.page.neo.accept', { credits: application.assessment.credits.toLocaleString() })
        : t('pages.admin.partners.page.neo.reject', {})}
    </Badge>
  ) : (
    <Text size='sm' c='dimmed'>
      {application.status === 'pending' && !application.assessmentError
        ? t('pages.admin.partners.page.neo.waiting', {})
        : t('pages.admin.partners.page.neo.none', {})}
    </Text>
  );

  return (
    <>
      <PartnerApplicationModal
        application={application}
        opened={openModal === 'view'}
        onClose={() => setOpenModal(null)}
      />
      <PartnerCreditsModal
        opened={openModal === 'approve' || openModal === 'credits'}
        onClose={() => setOpenModal(null)}
        title={
          openModal === 'credits'
            ? t('pages.admin.partners.page.modal.credits.title', { server: application.guildName })
            : t('pages.admin.partners.page.modal.approve.title', { server: application.guildName })
        }
        description={
          openModal === 'credits'
            ? t('pages.admin.partners.page.modal.credits.content', {})
            : t('pages.admin.partners.page.modal.approve.content', {})
        }
        confirm={
          openModal === 'credits'
            ? t('pages.admin.partners.page.button.saveCredits', {})
            : t('pages.admin.partners.page.button.approve', {})
        }
        initial={openModal === 'credits' ? application.monthlyCredits : (application.assessment?.credits ?? 0)}
        onConfirmed={(credits) =>
          openModal === 'credits'
            ? run({ action: 'credits', credits }, t('pages.admin.partners.page.toast.creditsChanged', {}))
            : run({ action: 'approve', credits }, t('pages.admin.partners.page.toast.approved', {}))
        }
      />
      <PartnerReasonModal
        opened={openModal === 'reject' || openModal === 'revoke' || openModal === 'pause'}
        onClose={() => setOpenModal(null)}
        title={
          openModal === 'revoke'
            ? t('pages.admin.partners.page.modal.revoke.title', { server: application.guildName })
            : openModal === 'pause'
              ? t('pages.admin.partners.page.modal.pause.title', { server: application.guildName })
              : t('pages.admin.partners.page.modal.reject.title', { server: application.guildName })
        }
        description={
          openModal === 'revoke'
            ? t('pages.admin.partners.page.modal.revoke.content', {})
            : openModal === 'pause'
              ? t('pages.admin.partners.page.modal.pause.content', {})
              : t('pages.admin.partners.page.modal.reject.content', {})
        }
        required={openModal !== 'pause'}
        confirm={
          openModal === 'revoke'
            ? t('pages.admin.partners.page.button.revoke', {})
            : openModal === 'pause'
              ? t('pages.admin.partners.page.button.pause', {})
              : t('pages.admin.partners.page.button.reject', {})
        }
        confirmColor={openModal === 'pause' ? 'blue' : 'red'}
        onConfirmed={(reason) =>
          openModal === 'revoke'
            ? run({ action: 'revoke', reason }, t('pages.admin.partners.page.toast.revoked', {}))
            : openModal === 'pause'
              ? run({ action: 'pause', reason }, t('pages.admin.partners.page.toast.paused', {}))
              : run({ action: 'reject', reason }, t('pages.admin.partners.page.toast.rejected', {}))
        }
      />

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
            label: t('pages.admin.partners.page.button.approve', {}),
            onClick: () => setOpenModal('approve'),
            canAccess: canManage && application.status === 'pending',
          },
          {
            type: 'action',
            icon: faArrowsRotate,
            label: t('pages.admin.partners.page.button.recheck', {}),
            onClick: recheck,
            canAccess: canManage && !checking && (application.status === 'pending' || partnership),
          },
          {
            type: 'action',
            icon: faCoins,
            label: t('pages.admin.partners.page.button.credits', {}),
            onClick: () => setOpenModal('credits'),
            canAccess: canManage && partnership,
          },
          {
            type: 'action',
            icon: faPause,
            label: t('pages.admin.partners.page.button.pause', {}),
            onClick: () => setOpenModal('pause'),
            canAccess: canManage && (application.status === 'approved' || application.status === 'suspended'),
          },
          {
            type: 'action',
            icon: faPlay,
            label: t('pages.admin.partners.page.button.resume', {}),
            onClick: () =>
              run({ action: 'resume' }, t('pages.admin.partners.page.toast.resumed', {})).catch((error) =>
                addToast(httpErrorToHuman(error), 'error'),
              ),
            canAccess: canManage && (application.status === 'paused' || application.status === 'suspended'),
          },
          {
            type: 'action',
            icon: faXmark,
            label: t('pages.admin.partners.page.button.reject', {}),
            onClick: () => setOpenModal('reject'),
            color: 'red',
            canAccess: canManage && application.status === 'pending',
          },
          {
            type: 'action',
            icon: faXmark,
            label: t('pages.admin.partners.page.button.revoke', {}),
            onClick: () => setOpenModal('revoke'),
            color: 'red',
            canAccess: canManage && partnership,
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
            <TableData>
              <Text size='sm' fw={600}>
                {application.guildName}
              </Text>
            </TableData>
            <TableData>
              {application.user ? (
                <TableLink to={`/admin/users/${application.user.uuid}`} onClick={(e) => e.stopPropagation()}>
                  {application.user.username}
                </TableLink>
              ) : (
                <Text size='sm' c='dimmed'>
                  {t('common.na', {})}
                </Text>
              )}
            </TableData>
            <TableData>{application.facts.members.toLocaleString()}</TableData>
            <TableData>
              <Badge color={failing ? 'red' : partnerStatusColorMapping[application.status]} variant='light'>
                {failing
                  ? t('pages.admin.partners.enum.status.failing', {})
                  : partnerStatusLabelMapping[application.status]()}
              </Badge>
            </TableData>
            <TableData>{neo}</TableData>
            <TableData>{partnership ? application.monthlyCredits.toLocaleString() : '—'}</TableData>
            <TableData>
              <FormattedTimestamp timestamp={application.created} />
            </TableData>

            <ContextMenuToggle items={items} openMenu={openMenu} />
          </TableRow>
        )}
      </ContextMenu>
    </>
  );
}
