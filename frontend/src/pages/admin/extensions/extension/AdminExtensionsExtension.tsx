import { faArrowLeft } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useMemo } from 'react';
import { Link, useParams } from 'react-router';
import AdminContentContainer from '@/elements/containers/AdminContentContainer.tsx';
import Text from '@/elements/typography/Text.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default function AdminExtensionsExtension() {
  const { t } = useTranslations();
  const { packageName } = useParams<'packageName'>();

  const extension = useMemo(() => {
    return window.extensionContext.extensions.find((ext) => ext.packageName === packageName);
  }, [packageName]);

  if (!extension) {
    return (
      <AdminContentContainer title={t('pages.admin.extensions.notFound.title', {})}>
        <span>{t('pages.admin.extensions.notFound.content', { packageName: packageName ?? '' })}</span>
      </AdminContentContainer>
    );
  }

  return (
    <AdminContentContainer title={t('pages.admin.extensions.configure.title', { packageName: extension.packageName })}>
      <Link to='/admin/extensions' className='inline-block mb-4 text-sm text-(--mantine-color-anchor) hover:underline'>
        <FontAwesomeIcon icon={faArrowLeft} /> {t('pages.admin.extensions.button.back', {})}
      </Link>

      {extension.cardConfigurationPage ? (
        <extension.cardConfigurationPage />
      ) : (
        <Text size='sm' c='dimmed'>
          {t('pages.admin.extensions.configure.noConfigurationPage', {})}
        </Text>
      )}
    </AdminContentContainer>
  );
}
