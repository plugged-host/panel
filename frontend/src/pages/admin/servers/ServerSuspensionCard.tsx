// plugged.host: a suspended server's details for admins (docs/features/suspension.md).
import { Text } from '@mantine/core';
import InfoRow from '@/elements/admin/InfoRow.tsx';
import TableLink from '@/elements/data-display/TableLink.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { formatDateTime } from '@/lib/format/time.ts';
import { AdminServer } from '@/lib/schemas/admin/servers.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default function ServerSuspensionCard({ server }: { server: AdminServer }) {
  const { t } = useTranslations();
  const suspension = server.suspension;

  if (!suspension) {
    return null;
  }

  return (
    <TitleCard title={t('pages.admin.servers.tabs.overview.page.suspension.title', {})}>
      <Stack gap={0}>
        <InfoRow label={t('pages.admin.servers.tabs.overview.page.suspension.label.kind', {})}>
          <Text size='sm'>{t(`pages.admin.servers.tabs.overview.page.suspension.kind.${suspension.kind}`, {})}</Text>
        </InfoRow>
        {suspension.suspendedBy && (
          <InfoRow label={t('pages.admin.servers.tabs.overview.page.suspension.label.suspendedBy', {})}>
            <TableLink to={`/admin/users/${suspension.suspendedBy}`}>
              {t('pages.admin.servers.tabs.overview.page.suspension.label.viewUser', {})}
            </TableLink>
          </InfoRow>
        )}
        <InfoRow label={t('pages.admin.servers.tabs.overview.page.suspension.label.since', {})}>
          <Text size='sm'>{suspension.suspended ? formatDateTime(suspension.suspended) : t('common.na', {})}</Text>
        </InfoRow>
        <InfoRow label={t('pages.admin.servers.tabs.overview.page.suspension.label.reason', {})}>
          <Text size='sm' className='whitespace-pre-line break-words'>
            {suspension.reason ?? t('common.na', {})}
          </Text>
        </InfoRow>
        <InfoRow label={t('pages.admin.servers.tabs.overview.page.suspension.label.deleteAt', {})}>
          <Text size='sm'>
            {suspension.deleteAt
              ? formatDateTime(suspension.deleteAt)
              : t('pages.admin.servers.tabs.overview.page.suspension.label.notDeleted', {})}
          </Text>
        </InfoRow>
      </Stack>
    </TitleCard>
  );
}
