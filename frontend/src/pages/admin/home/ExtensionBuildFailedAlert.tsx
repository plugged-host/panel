import { Link } from 'react-router';
import getExtensionStatus from '@/api/admin/extensions/manage/getExtensionStatus.ts';
import Alert from '@/elements/feedback/Alert.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useAdminCan } from '@/plugins/usePermissions.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default function ExtensionBuildFailedAlert({ className = 'mb-4' }: { className?: string }) {
  const { t } = useTranslations();
  const canManageExtensions = useAdminCan('extensions.manage');

  const { data: extensionStatus } = useResource({
    queryKey: queryKeys.admin.extensions.status(),
    queryFn: getExtensionStatus,
    enabled: canManageExtensions,
    silent: true,
  });

  if (extensionStatus?.supervisor?.state.type !== 'failed') {
    return null;
  }

  return (
    <Alert className={className} color='red'>
      {t('pages.admin.home.alert.extensionBuildFailed.content', {})}{' '}
      <Link to='/admin/extensions' className='text-(--mantine-color-anchor) hover:underline'>
        {t('pages.admin.home.alert.extensionBuildFailed.link', {})}
      </Link>
    </Alert>
  );
}
