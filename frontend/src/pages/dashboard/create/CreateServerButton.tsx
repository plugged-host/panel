// plugged.host: the server list's way to the create page, while players may create servers
import { useNavigate } from 'react-router';
import Button from '@/elements/buttons/Button.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

export default function CreateServerButton() {
  const { t } = useTranslations();
  const navigate = useNavigate();
  const createEnabled = useGlobalStore((state) => state.settings.games?.createEnabled);

  if (!createEnabled) {
    return null;
  }

  return (
    <Button data-tour='create-server' onClick={() => navigate('/create')}>
      {t('pages.account.createServer.button', {})}
    </Button>
  );
}
