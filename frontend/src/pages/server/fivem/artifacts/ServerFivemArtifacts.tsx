// plugged.host: the FXServer build a FiveM server runs and how it updates, laid out like the Pelican panel's Artifacts
// page: what is installed with the update switch, then every build Cfx offers as a row (docs/features/fivem.md).
// The Enhanced edition has its own page (enhanced/ServerFivemEnhanced.tsx).
import { faSearch } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQueryClient } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { NavLink } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import chooseFivemVersion from '@/api/server/fivem/chooseFivemVersion.ts';
import getFivemArtifacts from '@/api/server/fivem/getFivemArtifacts.ts';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import Notification from '@/elements/feedback/Notification.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { formatDateTime } from '@/lib/format/time.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { FivemVersionChoice, fivemArtifactsSchema, fivemOutcomeSchema } from '@/lib/schemas/server/fivem.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';

type Artifacts = z.infer<typeof fivemArtifactsSchema>;
type Mode = 'recommended' | 'latest' | 'pin';

const base = 'pages.server.fivem.artifacts';
const MODES: Mode[] = ['recommended', 'latest', 'pin'];

/** "It applies now" or "at the next start", the second line of every change's toast. */
export function outcomeText(
  t: ReturnType<typeof useTranslations>['t'],
  outcome: z.infer<typeof fivemOutcomeSchema>,
): string {
  return outcome === 'restarting' ? t(`${base}.toast.restarting`, {}) : t(`${base}.toast.nextStart`, {});
}

function Overview({ artifacts }: { artifacts: Artifacts }) {
  const { t, language } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const appName = useGlobalStore((state) => state.settings.app.name);
  const canChange = useServerCan('fivem.artifacts') && artifacts.changeable;
  const [saving, setSaving] = useState(false);
  const [installing, setInstalling] = useState<number | null>(null);
  const [search, setSearch] = useState('');

  const legacy = artifacts.edition === 'legacy';
  const installed = artifacts.installed;
  const installedBuild = installed?.edition === 'legacy' ? installed.build : null;
  const mode: Mode = artifacts.version === 'recommended' || artifacts.version === 'latest' ? artifacts.version : 'pin';
  const today = new Date().toISOString().slice(0, 10);

  const rows = useMemo(() => {
    const digits = search.replace(/\D/g, '');
    return digits ? artifacts.builds.filter((build) => String(build.build).includes(digits)) : artifacts.builds;
  }, [artifacts, search]);

  const refresh = () => queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).fivem.artifacts() });

  const choose = (choice: FivemVersionChoice) => {
    setSaving(true);

    return chooseFivemVersion(server.uuid, choice)
      .then((outcome) => {
        const title =
          choice.mode === 'build' || choice.mode === 'pin'
            ? t(`${base}.toast.build`, { build: choice.mode === 'build' ? choice.build : (installedBuild ?? '') })
            : t(`${base}.toast.channel`, { channel: t(`${base}.channel.${choice.mode}`, {}) });
        addToast(`${title} ${outcomeText(t, outcome)}`, 'success');
        setInstalling(null);
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
      })
      .finally(() => {
        setSaving(false);
        refresh();
      });
  };

  const supportDate = (date: string) =>
    new Date(`${date}T00:00:00`).toLocaleDateString(language, { year: 'numeric', month: 'short', day: 'numeric' });
  const editionLabel = (edition: Artifacts['edition']) => t(`pages.server.fivem.enhanced.edition.${edition}`, {});

  return (
    <div className='flex flex-col gap-6'>
      <ConfirmationModal
        opened={installing !== null}
        onClose={() => setInstalling(null)}
        title={t(`${base}.install.title`, { build: installing ?? '' })}
        confirm={t(`${base}.install.button`, {})}
        confirmColor='blue'
        onConfirmed={() => (installing !== null ? choose({ mode: 'build', build: installing }) : undefined)}
      >
        {t(`${base}.install.description`, {})}
      </ConfirmationModal>

      {!legacy && (
        <Notification className='theme-callout' color='blue'>
          <p className='theme-eco-text'>{t(`${base}.enhancedNote`, {})}</p>
          <NavLink
            to={`/server/${server.uuidShort}/fivem/enhanced`}
            className='theme-eco-link mt-2 inline-block text-sm'
          >
            {t(`${base}.enhancedLink`, {})}
          </NavLink>
        </Notification>
      )}

      <div className='theme-fivem-now'>
        <div>
          <div className='theme-fivem-label'>{t(`${base}.installed.title`, {})}</div>
          <div className='theme-fivem-big'>
            {artifacts.unreachable
              ? t(`${base}.installed.unreachable`, {})
              : installed === null
                ? t(`${base}.installed.unknown`, {})
                : legacy && installedBuild !== null
                  ? t(`${base}.build`, { build: installedBuild })
                  : installed.build !== null
                    ? `${editionLabel(installed.edition)} · ${t(`${base}.build`, { build: installed.build })}`
                    : editionLabel(installed.edition)}
          </div>
          {installed?.installedAt ? (
            <div className='theme-eco-muted'>
              {t(`${base}.installed.since`, { when: formatDateTime(installed.installedAt) })}
            </div>
          ) : (
            installed === null &&
            !artifacts.unreachable && (
              <div className='theme-eco-muted'>{t(`${base}.installed.unknownDescription`, {})}</div>
            )
          )}
          {artifacts.pending !== null && !artifacts.unreachable && (
            <div className='theme-fivem-pending'>{t(`${base}.pending`, { build: artifacts.pending })}</div>
          )}
        </div>
        {legacy && (
          <div>
            <div className='theme-fivem-label mb-1.5'>{t(`${base}.mode.title`, {})}</div>
            <div className='theme-fivem-seg' role='group' aria-label={t(`${base}.mode.title`, {})}>
              {MODES.map((option) => (
                <button
                  key={option}
                  type='button'
                  aria-pressed={mode === option}
                  title={t(`${base}.mode.${option}.description`, {})}
                  disabled={!canChange || saving || mode === option || (option === 'pin' && installedBuild === null)}
                  onClick={() => choose({ mode: option })}
                >
                  {t(`${base}.mode.${option}.label`, {})}
                </button>
              ))}
            </div>
          </div>
        )}
      </div>

      {legacy && (
        <>
          <div className='flex flex-wrap items-center justify-between gap-3'>
            <TextInput
              type='search'
              inputMode='numeric'
              aria-label={t(`${base}.search`, {})}
              placeholder={t(`${base}.search`, {})}
              leftSection={<FontAwesomeIcon icon={faSearch} />}
              value={search}
              onChange={(e) => setSearch(e.currentTarget.value)}
              className='w-full sm:max-w-80'
            />
            <span className='theme-eco-muted'>
              {t(`${base}.count.${rows.length === 0 ? 'none' : rows.length === 1 ? 'one' : 'other'}`, {
                count: rows.length,
              })}
            </span>
          </div>

          {rows.length === 0 ? (
            <div className='theme-fivem-empty'>
              {search.trim() ? t(`${base}.noMatch`, {}) : t(`${base}.cfxUnreachable`, {})}
            </div>
          ) : (
            <div className='flex flex-col gap-2'>
              {rows.map((build) => {
                const ended = build.supportEnds !== null && build.supportEnds < today;

                return (
                  <div key={build.build} className='theme-fivem-build'>
                    <div className='min-w-0'>
                      <div className='flex flex-wrap items-center gap-2'>
                        <span className='theme-fivem-num'>{t(`${base}.build`, { build: build.build })}</span>
                        {build.recommended && <Badge color='green'>{t(`${base}.badge.recommended`, {})}</Badge>}
                        {build.latest && <Badge color='blue'>{t(`${base}.badge.latest`, {})}</Badge>}
                        {build.installed && <Badge color='gray'>{t(`${base}.badge.installed`, {})}</Badge>}
                        {ended && <Badge color='yellow'>{t(`${base}.unsupported`, {})}</Badge>}
                      </div>
                      {build.supportEnds !== null && !ended && (
                        <div className='theme-eco-muted'>
                          {t(`${base}.supportedUntil`, { date: supportDate(build.supportEnds) })}
                        </div>
                      )}
                    </div>
                    {canChange && !(build.installed && build.selected) && (
                      <Button variant='default' size='sm' disabled={saving} onClick={() => setInstalling(build.build)}>
                        {t(`${base}.install.button`, {})}
                      </Button>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </>
      )}

      <p className='theme-eco-muted'>{t(`${base}.sourceNote`, { app: appName })}</p>
    </div>
  );
}

export default function ServerFivemArtifacts() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);

  const artifacts = useResource({
    queryKey: queryKeys.server(server.uuid).fivem.artifacts(),
    queryFn: () => getFivemArtifacts(server.uuid),
  });

  return (
    <ServerContentContainer title={t(`${base}.title`, {})} subtitle={t(`${base}.subtitle`, {})}>
      <ResourceView resource={artifacts}>{(data) => <Overview artifacts={data} />}</ResourceView>
    </ServerContentContainer>
  );
}
