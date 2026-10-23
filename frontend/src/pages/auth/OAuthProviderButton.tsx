import { faFingerprint } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { z } from 'zod';
import Button from '@/elements/buttons/Button.tsx';
import { useCustomIcon } from '@/lib/icons.ts';
import { oAuthProviderSchema } from '@/lib/schemas/generic.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default function OAuthProviderButton({ oAuthProvider }: { oAuthProvider: z.infer<typeof oAuthProviderSchema> }) {
  const { t } = useTranslations();
  const icon = useCustomIcon(oAuthProvider.icon, faFingerprint);

  return (
    <Button
      leftSection={<FontAwesomeIcon icon={icon} />}
      size='md'
      fullWidth
      onClick={() => {
        window.location.href = `/api/auth/oauth/redirect/${oAuthProvider.uuid}`;
      }}
    >
      {t('pages.auth.button.loginWith', { name: oAuthProvider.name })}
    </Button>
  );
}
