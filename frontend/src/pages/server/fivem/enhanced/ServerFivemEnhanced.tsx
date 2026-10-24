// plugged.host: FiveM for GTAV Enhanced, its own page like the Pelican panel's: the edition the server runs with the one
// switch, then what changes for players, resources and txAdmin (docs/features/fivem.md). Moving to Enhanced needs Admin ›
// Settings › FiveM for GTAV Enhanced (it ships off); going back to Legacy never does.
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { z } from 'zod';
import getFivemArtifacts from '@/api/server/fivem/getFivemArtifacts.ts';
import switchFivemEdition from '@/api/server/fivem/switchFivemEdition.ts';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import Notification from '@/elements/feedback/Notification.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { fivemArtifactsSchema } from '@/lib/schemas/server/fivem.ts';
import { outcomeText } from '@/pages/server/fivem/artifacts/ServerFivemArtifacts.tsx';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';

type Artifacts = z.infer<typeof fivemArtifactsSchema>;

const base = 'pages.server.fivem.enhanced';
const FACTS = ['client', 'scripts', 'assets', 'network', 'switching', 'txadmin'] as const;

function Overview({ artifacts }: { artifacts: Artifacts }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const appName = useGlobalStore((state) => state.settings.app.name);
  const canChange = useServerCan('fivem.artifacts');
  const [confirming, setConfirming] = useState(false);

  const target = artifacts.edition === 'enhanced' ? 'legacy' : 'enhanced';
  const shown = artifacts.installed?.edition ?? artifacts.edition;
  const pending = artifacts.installed !== null && artifacts.installed.edition !== artifacts.edition;
  const installedBuild = artifacts.installed?.build ?? null;
  // Going back to Legacy never depends on the setting; a server set to Enhanced has the edition variable.
  const canSwitch = !artifacts.redm && canChange && (target === 'legacy' || artifacts.enhancedAvailable);
  const targetKey = target === 'enhanced' ? 'Enhanced' : 'Legacy';

  const doSwitch = () =>
    switchFivemEdition(server.uuid, target).then((outcome) => {
      addToast(`${t(`${base}.switched${targetKey}`, {})} ${outcomeText(t, outcome)}`, 'success');
      setConfirming(false);
      queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).fivem.artifacts() });
    });

  return (
    <div className='flex flex-col gap-6'>
      <ConfirmationModal
        opened={confirming}
        onClose={() => setConfirming(false)}
        title={t(`${base}.confirm${targetKey}`, {})}
        confirm={t(`${base}.switchTo${targetKey}`, {})}
        confirmColor='blue'
        onConfirmed={doSwitch}
      >
        {t(`${base}.confirm${targetKey}Description`, {})}
      </ConfirmationModal>

      {artifacts.redm ? (
        <Notification className='theme-callout' color='blue'>
          {t(`${base}.redm`, {})}
        </Notification>
      ) : (
        <>
          {artifacts.enhancedAvailable ? (
            <Notification className='theme-callout' color='yellow'>
              {t(`${base}.earlyAccess`, {})}
            </Notification>
          ) : (
            <Notification className='theme-callout' color='blue'>
              {t(`${base}.unavailable`, { app: appName })}
            </Notification>
          )}

          <div className='theme-fivem-now'>
            <div>
              <div className='theme-fivem-big'>{t(`${base}.edition.${shown}`, {})}</div>
              <div className='theme-eco-muted mt-0.5'>
                {shown === 'enhanced'
                  ? installedBuild !== null
                    ? t(`${base}.runsEnhancedBuild`, { build: installedBuild })
                    : t(`${base}.runsEnhanced`, {})
                  : t(`${base}.runsLegacy`, {})}
              </div>
              {pending && (
                <div className='theme-fivem-pending'>
                  {artifacts.edition === 'enhanced' ? t(`${base}.pendingEnhanced`, {}) : t(`${base}.pendingLegacy`, {})}
                </div>
              )}
            </div>
            {canSwitch && (
              <div>
                <Button onClick={() => setConfirming(true)}>{t(`${base}.switchTo${targetKey}`, {})}</Button>
              </div>
            )}
          </div>
        </>
      )}

      <div>
        <div className='theme-fivem-label px-1 pb-2'>{t(`${base}.compatibility`, {})}</div>
        <div>
          {FACTS.map((fact) => (
            <div key={fact} className='theme-fivem-fact'>
              <strong>{t(`${base}.facts.${fact}.title`, {})}</strong>
              <p>{t(`${base}.facts.${fact}.description`, { build: artifacts.enhancedBuild ?? '?' })}</p>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

export default function ServerFivemEnhanced() {
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
