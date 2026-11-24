// plugged.host: the server list's way to the create page, while players may create servers, and to
// the Paid page with its lowest price while a paid plan is on sale (docs/features/dedicated.md)
import { useNavigate } from 'react-router';
import Button from '@/elements/buttons/Button.tsx';
import usePaidOffer from '@/pages/dashboard/dedicated/usePaidOffer.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

export default function CreateServerButton({ centered = false }: { centered?: boolean }) {
  const { t } = useTranslations();
  const navigate = useNavigate();
  const createEnabled = useGlobalStore((state) => state.settings.games?.createEnabled);
  const { offered, from } = usePaidOffer();

  if (!createEnabled && !offered) {
    return null;
  }

  return (
    <div className={`flex flex-wrap items-center gap-2${centered ? ' justify-center' : ''}`}>
      {createEnabled && (
        <Button data-tour='create-server' onClick={() => navigate('/create')}>
          {t('pages.account.createServer.button', {})}
        </Button>
      )}
      {offered && (
        <Button variant='default' onClick={() => navigate('/account/dedicated')}>
          {t('pages.account.createServer.paidButton', {})}
        </Button>
      )}
      {offered && from && (
        <span className='theme-eco-text'>{t('pages.account.dedicated.upsell.from', { price: from })}</span>
      )}
    </div>
  );
}
