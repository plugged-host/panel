// plugged.host: the FXServer build a FiveM server runs, how it updates, and its edition (docs/features/fivem.md)
import { useQueryClient } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import chooseFivemVersion from '@/api/server/fivem/chooseFivemVersion.ts';
import getFivemArtifacts from '@/api/server/fivem/getFivemArtifacts.ts';
import switchFivemEdition from '@/api/server/fivem/switchFivemEdition.ts';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import Table, { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import Choice, { ChoiceSection } from '@/elements/input/Choice.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import Text from '@/elements/typography/Text.tsx';
import Title from '@/elements/typography/Title.tsx';
import { formatTimestamp } from '@/lib/format/time.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import {
  FivemVersionChoice,
  fivemArtifactsSchema,
  fivemEditionSchema,
  fivemOutcomeSchema,
} from '@/lib/schemas/server/fivem.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

type Artifacts = z.infer<typeof fivemArtifactsSchema>;
type Edition = z.infer<typeof fivemEditionSchema>;

const base = 'pages.server.fivem.artifacts';
const FACTS = ['client', 'scripts', 'assets', 'network', 'switching', 'txadmin'] as const;

function EditionSection({ artifacts, onChanged }: { artifacts: Artifacts; onChanged: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const [confirming, setConfirming] = useState<Edition | null>(null);

  if (artifacts.redm) {
    return (
      <Text size='sm' c='dimmed'>
        {t(`${base}.edition.redm`, {})}
      </Text>
    );
  }

  const enhanced = artifacts.edition === 'enhanced';
  if (!enhanced && !artifacts.enhancedAvailable) return null;

  const switchTo = (edition: Edition) =>
    switchFivemEdition(server.uuid, edition).then((outcome) => {
      const done =
        edition === 'enhanced' ? t(`${base}.edition.switchedEnhanced`, {}) : t(`${base}.edition.switchedLegacy`, {});
      addToast(`${done} ${outcomeText(t, outcome)}`, 'success');
      setConfirming(null);
      onChanged();
    });

  return (
    <Stack gap='sm'>
      <ConfirmationModal
        opened={confirming !== null}
        onClose={() => setConfirming(null)}
        title={
          confirming === 'enhanced' ? t(`${base}.edition.confirmEnhanced`, {}) : t(`${base}.edition.confirmLegacy`, {})
        }
        confirm={
          confirming === 'enhanced'
            ? t(`${base}.edition.switchToEnhanced`, {})
            : t(`${base}.edition.switchToLegacy`, {})
        }
        confirmColor='blue'
        onConfirmed={() => (confirming ? switchTo(confirming) : undefined)}
      >
        {confirming === 'enhanced'
          ? t(`${base}.edition.confirmEnhancedDescription`, {})
          : t(`${base}.edition.confirmLegacyDescription`, {})}
      </ConfirmationModal>

      <Title order={3} fz='md' fw={600}>
        {t(`${base}.edition.title`, {})}
      </Title>
      <Text size='sm'>
        {enhanced
          ? artifacts.enhancedBuild !== null
            ? t(`${base}.edition.runsEnhancedBuild`, { build: artifacts.enhancedBuild })
            : t(`${base}.edition.runsEnhanced`, {})
          : t(`${base}.edition.runsLegacy`, {})}
      </Text>
      <Text size='sm' c='dimmed'>
        {t(`${base}.edition.earlyAccess`, {})}
      </Text>

      <Stack gap='xs'>
        {FACTS.map((fact) => (
          <Stack key={fact} gap={0}>
            <Text size='sm' fw={500}>
              {t(`${base}.edition.facts.${fact}.title`, {})}
            </Text>
            <Text size='sm' c='dimmed'>
              {t(`${base}.edition.facts.${fact}.description`, { build: artifacts.enhancedBuild ?? '?' })}
            </Text>
          </Stack>
        ))}
      </Stack>

      <Group>
        <Button variant='default' onClick={() => setConfirming(enhanced ? 'legacy' : 'enhanced')}>
          {enhanced ? t(`${base}.edition.switchToLegacy`, {}) : t(`${base}.edition.switchToEnhanced`, {})}
        </Button>
      </Group>
    </Stack>
  );
}

function outcomeText(t: ReturnType<typeof useTranslations>['t'], outcome: z.infer<typeof fivemOutcomeSchema>): string {
  return outcome === 'restarting' ? t(`${base}.toast.restarting`, {}) : t(`${base}.toast.nextStart`, {});
}

function Overview({ artifacts, search }: { artifacts: Artifacts; search: string }) {
  const { t, language } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const canChange = useServerCan('fivem.artifacts');
  const [saving, setSaving] = useState(false);
  const [installing, setInstalling] = useState<number | null>(null);

  const refresh = () => queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).fivem.artifacts() });

  const choose = (choice: FivemVersionChoice) => {
    setSaving(true);

    return chooseFivemVersion(server.uuid, choice)
      .then((outcome) => {
        const message =
          choice.mode === 'build'
            ? `${t(`${base}.toast.build`, { build: choice.build })} ${outcomeText(t, outcome)}`
            : t(`${base}.toast.mode`, { mode: t(`${base}.mode.${choice.mode}.label`, {}) });
        addToast(message, 'success');
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

  const rows = useMemo(() => {
    const query = search.trim();
    return query ? artifacts.builds.filter((build) => String(build.build).includes(query)) : artifacts.builds;
  }, [artifacts, search]);

  const mode = artifacts.version === 'recommended' || artifacts.version === 'latest' ? artifacts.version : 'pin';
  const legacy = artifacts.edition === 'legacy';
  const supportDate = (date: string) =>
    new Date(`${date}T00:00:00`).toLocaleDateString(language, { year: 'numeric', month: 'short', day: 'numeric' });
  const today = new Date().toISOString().slice(0, 10);
  const installed = artifacts.installed;

  return (
    <Stack gap='xl'>
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

      <Stack gap='xs'>
        {!artifacts.onImage && (
          <Text size='sm' c='dimmed'>
            {t(`${base}.notOnImage`, {})}
          </Text>
        )}
        {!artifacts.changeable && (
          <Text size='sm' c='dimmed'>
            {t(`${base}.notChangeable`, {})}
          </Text>
        )}

        <Stack gap={0}>
          <Text size='sm' fw={500}>
            {t(`${base}.installed.title`, {})}
          </Text>
          <Text size='sm'>
            {artifacts.unreachable
              ? t(`${base}.installed.unreachable`, {})
              : installed === null
                ? t(`${base}.installed.none`, {})
                : installed.build === null
                  ? t(`${base}.installed.unknown`, {})
                  : `${installed.edition === 'enhanced' ? t(`${base}.edition.enhanced`, {}) : t(`${base}.edition.legacy`, {})} · ${t(`${base}.build`, { build: installed.build })}`}
            {installed?.installedAt
              ? ` · ${t(`${base}.installed.since`, { when: formatTimestamp(installed.installedAt) })}`
              : ''}
          </Text>
          {artifacts.pending !== null && (
            <Text size='sm' c='dimmed'>
              {t(`${base}.pending`, { build: artifacts.pending })}
            </Text>
          )}
        </Stack>
      </Stack>

      {legacy && artifacts.changeable && (
        <ChoiceSection title={t(`${base}.mode.title`, {})}>
          <div className='theme-choice-grid'>
            {(['recommended', 'latest', 'pin'] as const).map((choice) => (
              <Choice
                key={choice}
                selected={mode === choice}
                disabled={!canChange || saving}
                onSelect={() => {
                  if (mode !== choice) choose({ mode: choice });
                }}
              >
                <span className='theme-choice-title'>{t(`${base}.mode.${choice}.label`, {})}</span>
                <span className='theme-choice-text'>{t(`${base}.mode.${choice}.description`, {})}</span>
              </Choice>
            ))}
          </div>
        </ChoiceSection>
      )}

      {legacy && (
        <Stack gap='sm'>
          <Title order={3} fz='md' fw={600}>
            {t(`${base}.builds`, {})}
          </Title>
          <Table
            columns={[t(`${base}.table.build`, {}), t(`${base}.table.support`, {}), '']}
            pagination={{ total: rows.length, perPage: Math.max(rows.length, 1), page: 1, data: rows }}
            empty={
              <Text size='sm' c='dimmed'>
                {search.trim() ? t(`${base}.noMatch`, {}) : t(`${base}.cfxUnreachable`, {})}
              </Text>
            }
          >
            {rows.map((build) => (
              <TableRow key={build.build}>
                <TableData>
                  <Group gap='xs'>
                    <Text size='sm'>{build.build}</Text>
                    {build.recommended && (
                      <Badge color='green' size='xs'>
                        {t(`${base}.badge.recommended`, {})}
                      </Badge>
                    )}
                    {build.latest && (
                      <Badge color='blue' size='xs'>
                        {t(`${base}.badge.latest`, {})}
                      </Badge>
                    )}
                    {build.installed && (
                      <Badge color='gray' size='xs'>
                        {t(`${base}.badge.installed`, {})}
                      </Badge>
                    )}
                  </Group>
                </TableData>
                <TableData>
                  {build.supportEnds === null ? (
                    ''
                  ) : build.supportEnds < today ? (
                    <Text size='sm' c='red'>
                      {t(`${base}.unsupported`, {})}
                    </Text>
                  ) : (
                    <Text size='sm' c='dimmed'>
                      {t(`${base}.supportedUntil`, { date: supportDate(build.supportEnds) })}
                    </Text>
                  )}
                </TableData>
                <TableData className='text-end'>
                  {canChange && artifacts.changeable && !build.selected && (
                    <Button variant='default' size='xs' disabled={saving} onClick={() => setInstalling(build.build)}>
                      {t(`${base}.install.button`, {})}
                    </Button>
                  )}
                </TableData>
              </TableRow>
            ))}
          </Table>
        </Stack>
      )}

      {canChange && <EditionSection artifacts={artifacts} onChanged={refresh} />}

      <Text size='xs' c='dimmed'>
        {t(`${base}.sourceNote`, {})}
      </Text>
    </Stack>
  );
}

export default function ServerFivemArtifacts() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  const [search, setSearch] = useState('');

  const artifacts = useResource({
    queryKey: queryKeys.server(server.uuid).fivem.artifacts(),
    queryFn: () => getFivemArtifacts(server.uuid),
  });

  return (
    <ServerContentContainer title={t(`${base}.title`, {})} search={search} setSearch={setSearch}>
      <ResourceView resource={artifacts}>{(data) => <Overview artifacts={data} search={search} />}</ResourceView>
    </ServerContentContainer>
  );
}
