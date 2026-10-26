// plugged.host: how far the modpack install got, the Pelican panel's modpack notifications as a callout (docs/features/minecraft.md)
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useRef } from 'react';
import Notification from '@/elements/feedback/Notification.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { useDismissed } from '../useDismissed.ts';
import ModpackProgress from './ModpackProgress.tsx';
import { ModpackInstall, modpackInstalling, useModpacks } from './useModpacks.ts';

const base = 'pages.server.minecraft.modpacks';

function Running({ install }: { install: ModpackInstall }) {
  const { t } = useTranslations();

  return (
    <Notification className='theme-callout' loading title={t(`${base}.state.queuedHeading`, {})}>
      <Stack gap='xs'>
        <span>{t(`${base}.state.queued`, { name: install.name })}</span>
        <ModpackProgress install={install} />
      </Stack>
    </Notification>
  );
}

function Ended({ install, onClose }: { install: ModpackInstall; onClose: () => void }) {
  const { t } = useTranslations();
  const done = install.step === 'done';

  return (
    <Notification
      className='theme-callout'
      color={done ? 'green' : 'red'}
      title={t(done ? `${base}.state.doneHeading` : `${base}.state.failedHeading`, {})}
      withCloseButton
      closeButtonProps={{ 'aria-label': t('common.button.close', {}) }}
      onClose={onClose}
    >
      <Stack gap='xs'>
        <span>
          {done
            ? t(`${base}.state.done`, { name: install.name, count: install.files })
            : t(`${base}.state.failed`, { name: install.name, error: install.error ?? '' })}
        </span>
        {install.skipped.length > 0 && <span>{t(`${base}.state.skipped`, { files: install.skipped.join(', ') })}</span>}
        {install.overridesSkipped && <span>{t(`${base}.state.overridesSkipped`, {})}</span>}
      </Stack>
    </Notification>
  );
}

export default function ModpackInstallCallout() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);

  const modpacks = useModpacks();
  const install = modpacks.data?.install ?? null;
  const [dismissed, dismiss] = useDismissed(
    install && !modpackInstalling(install) ? `modpack:${server.uuid}:${install.started.toISOString()}` : null,
  );

  // the install runs after its request: say so when it ends while the page is open
  const step = install?.step;
  const previous = useRef(step);
  useEffect(() => {
    if (install && previous.current && previous.current !== 'done' && previous.current !== 'failed') {
      if (step === 'done') {
        addToast(t(`${base}.state.done`, { name: install.name, count: install.files }), 'success');
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.installed() });
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.content() });
      } else if (step === 'failed') {
        addToast(t(`${base}.state.failed`, { name: install.name, error: install.error ?? '' }), 'error');
      }
    }
    previous.current = step;
  }, [step]);

  if (!install) return null;
  if (modpackInstalling(install)) return <Running install={install} />;
  if (dismissed) return null;

  return <Ended install={install} onClose={dismiss} />;
}
