// plugged.host: the server list's way to the create page, while players may create servers, and to
// the paid plans while they are offered; greyed out, with a hint, for an account without
// noprofit.space Funds (docs/features/dedicated.md)
import { useQuery } from '@tanstack/react-query';
import { useNavigate } from 'react-router';
import getBalance from '@/api/me/economy/getBalance.ts';
import Button from '@/elements/buttons/Button.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

export default function CreateServerButton() {
  const { t } = useTranslations();
  const navigate = useNavigate();
  const createEnabled = useGlobalStore((state) => state.settings.games?.createEnabled);
  const dedicatedOffered = useGlobalStore((state) => state.settings.games?.dedicatedOffered);

  // the sidebar's balance query: the same cache, no second request
  const { data: balance } = useQuery({
    queryKey: queryKeys.user.balance(),
    queryFn: getBalance,
    refetchInterval: 60_000,
    enabled: !!dedicatedOffered,
  });
  // Funds are noprofit.space's (null without it): without them the paid plans cannot be paid for
  const needsFunds = balance?.funds !== null && balance?.funds !== undefined && balance.funds <= 0;

  if (!createEnabled && !dedicatedOffered) {
    return null;
  }

  return (
    <div className='flex flex-wrap items-center gap-2'>
      {createEnabled && (
        <Button data-tour='create-server' onClick={() => navigate('/create')}>
          {t('pages.account.createServer.button', {})}
        </Button>
      )}
      {dedicatedOffered && (
        <Tooltip label={t('pages.account.createServer.paidNeedsFunds', {})} disabled={!needsFunds}>
          <span>
            <Button variant='default' disabled={needsFunds} onClick={() => navigate('/account/dedicated')}>
              {t('pages.account.createServer.paidButton', {})}
            </Button>
          </span>
        </Tooltip>
      )}
    </div>
  );
}
