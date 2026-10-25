// plugged.host: back to a devbox in the browser: the edge sends a browser without a valid
// sign-in here (docs/features/devboxes.md). Like the Pelican panel's redirect: a fresh link, the
// server's VS Code page, or the server list when the devbox is gone.
import { useEffect, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router';
import { httpErrorToHuman } from '@/api/axios.ts';
import reopenDevbox from '@/api/me/devboxes/reopenDevbox.ts';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import Text from '@/elements/typography/Text.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

const page = 'pages.account.devboxOpen';

export default function DashboardDevboxOpen() {
  const { t } = useTranslations();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const [error, setError] = useState<string | null>(null);
  const host = params.get('host') ?? '';

  useEffect(() => {
    if (!host) {
      navigate('/', { replace: true });
      return;
    }

    reopenDevbox(host)
      .then(({ url, server }) => {
        if (url) {
          window.location.replace(url);
        } else {
          navigate(server ? `/server/${server}/devbox` : '/', { replace: true });
        }
      })
      .catch((err) => setError(httpErrorToHuman(err)));
  }, [host]);

  return (
    <AccountContentContainer title={t(`${page}.title`, {})}>
      {error ? (
        <Stack gap='xs' align='flex-start'>
          <Text>{error}</Text>
          <Button variant='default' onClick={() => navigate('/')}>
            {t(`${page}.button.servers`, {})}
          </Button>
        </Stack>
      ) : (
        <Group gap='sm'>
          <Spinner size={20} />
          <Text>{t(`${page}.opening`, {})}</Text>
        </Group>
      )}
    </AccountContentContainer>
  );
}
