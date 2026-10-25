// plugged.host: the automatic log check above the console, the way Aternos does it: the log goes
// to mclo.gs and the problems it recognises show here (docs/features/minecraft.md). The look is
// the Pelican panel's log check (plugged-host-ai's log-check.blade.php, ".theme-logcheck-*" in
// app.css). "Ask neo to fix it" sends the window event `neo:ask` with the log's mclo.gs id, which
// the neo chat listens for (docs/features/neo.md).
import { useQuery } from '@tanstack/react-query';
import getLogCheck from '@/api/server/minecraft/getLogCheck.ts';
import Button from '@/elements/buttons/Button.tsx';
import { hasMinecraftLogs } from '@/lib/domain/games.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';

const SHOWN = 3;

export default function LogCheckBanner() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  // the check publishes the log, so it needs what reading the log needs
  const canCheck = useServerCan(['control.console', 'files.read-content'], false);
  // "Ask neo to fix it": neo is offered and may work on this server
  const neoEnabled = useGlobalStore((state) => state.settings.neo.enabled);
  const canUseNeo = useServerCan('neo.access');
  const canAsk = neoEnabled && canUseNeo;
  // every edition, Velocity and Bedrock included (shared/src/minecraft/mod.rs `require_any`)
  const minecraft = hasMinecraftLogs(server.egg);
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
      <p className='theme-logcheck-quiet theme-logcheck-block'>
        {t(`${base}.clean`, {})}{' '}
        <a href={result.url} target='_blank' rel='noopener noreferrer' className='theme-logcheck-link'>
          {t(`${base}.open`, {})}
        </a>
      </p>
    );
  }

  const problems = result.problems.length;
  const shown = result.problems.slice(0, SHOWN);
  const title =
    problems > 0
      ? problems === 1
        ? t(`${base}.problemsOne`, {})
        : t(`${base}.problems`, { count: problems })
      : result.errors === 1
        ? t(`${base}.errorsOne`, {})
        : t(`${base}.errors`, { count: result.errors });

  return (
    <div className='theme-logcheck-card theme-logcheck-block' role='status'>
      <div className='min-w-0'>
        <p className='theme-logcheck-title'>{title}</p>

        {shown.length > 0 ? (
          <>
            <ul className='theme-logcheck-list'>
              {shown.map((problem) => (
                <li key={problem.message} className='theme-logcheck-problem'>
                  {problem.message}
                  {problem.solution && (
                    <span className='theme-logcheck-fix'>{t(`${base}.fix`, { fix: problem.solution })}</span>
                  )}
                </li>
              ))}
            </ul>
            {problems > shown.length && (
              <p className='theme-logcheck-quiet'>{t(`${base}.more`, { count: problems - shown.length })}</p>
            )}
          </>
        ) : (
          <p className='theme-logcheck-quiet'>
            {canAsk ? t(`${base}.errorsBody`, {}) : t(`${base}.errorsBodyNoNeo`, {})}
          </p>
        )}
      </div>

      <div className='theme-logcheck-actions'>
        {canAsk && (
          <Button
            variant='default'
            onClick={() => {
              window.dispatchEvent(new CustomEvent('neo:ask', { detail: { log: result.id } }));
            }}
          >
            {t(`${base}.ask`, {})}
          </Button>
        )}
        <a
          href={result.url}
          target='_blank'
          rel='noopener noreferrer'
          className='theme-logcheck-quiet theme-logcheck-link'
        >
          {t(`${base}.view`, {})}
        </a>
      </div>
    </div>
  );
}
