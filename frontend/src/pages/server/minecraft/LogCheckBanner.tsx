// plugged.host: the automatic log check above the console, the way Aternos does it: the log goes
// to mclo.gs and the problems it recognises show here (docs/features/minecraft.md)
import { useQuery } from '@tanstack/react-query';
import getLogCheck from '@/api/server/minecraft/getLogCheck.ts';
import Card from '@/elements/data-display/Card.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import Anchor from '@/elements/typography/Anchor.tsx';
import Text from '@/elements/typography/Text.tsx';
import { isOtherGameRoute } from '@/lib/domain/games.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

const SHOWN = 3;

export default function LogCheckBanner() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  // the check publishes the log, so it needs what reading the log needs
  const canCheck = useServerCan(['control.console', 'files.read-content'], false);
  const minecraft = !isOtherGameRoute('/minecraft', server.egg?.features);
  const base = 'pages.server.minecraft.logCheck';

  const { data } = useQuery({
    queryKey: queryKeys.server(server.uuid).minecraft.logCheck(),
    queryFn: () => getLogCheck(server.uuid),
    enabled: minecraft && canCheck,
    refetchInterval: 60_000,
    retry: false,
  });

  const result = data?.enabled ? data.result : null;
  if (!result) return null;

  if (result.problems.length === 0 && result.errors === 0) {
    return (
      <Text size='sm' c='dimmed' mb='md'>
        {t(`${base}.clean`, {})}{' '}
        <Anchor href={result.url} target='_blank' rel='noopener noreferrer'>
          {t(`${base}.open`, {})}
        </Anchor>
      </Text>
    );
  }

  const shown = result.problems.slice(0, SHOWN);

  return (
    <Card mb='md' role='status'>
      <Stack gap='sm'>
        <Text size='sm' fw={600} c='yellow'>
          {result.problems.length > 0
            ? t(`${base}.problems`, { count: result.problems.length })
            : t(`${base}.errors`, { count: result.errors })}
        </Text>

        {shown.length > 0 ? (
          <Stack gap='xs'>
            {shown.map((problem) => (
              <div key={problem.message}>
                <Text size='sm'>{problem.message}</Text>
                {problem.solution && (
                  <Text size='xs' c='dimmed'>
                    {t(`${base}.fix`, { fix: problem.solution })}
                  </Text>
                )}
              </div>
            ))}
            {result.problems.length > shown.length && (
              <Text size='xs' c='dimmed'>
                {t(`${base}.more`, { count: result.problems.length - shown.length })}
              </Text>
            )}
          </Stack>
        ) : (
          <Text size='sm' c='dimmed'>
            {t(`${base}.errorsBody`, {})}
          </Text>
        )}

        <Anchor href={result.url} target='_blank' rel='noopener noreferrer' size='sm'>
          {t(`${base}.view`, {})}
        </Anchor>
      </Stack>
    </Card>
  );
}
