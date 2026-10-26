// plugged.host: the console's "Your server is not set up yet" callout (the Pelican panel's
// onboarding-callout), and how applying the answers goes once they are given (docs/features/minecraft.md)
import { NavLink } from 'react-router';
import Button from '@/elements/buttons/Button.tsx';
import Notification from '@/elements/feedback/Notification.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { useDismissed } from '../useDismissed.ts';
import SetupProgress from './SetupProgress.tsx';
import { SETUP_PATH, setupRunning, useMinecraftSetup } from './useMinecraftSetup.ts';

const base = 'pages.server.minecraft.setup';

export default function SetupCallout() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  const canSetUp = useServerCan('minecraft.settings');
  const setup = useMinecraftSetup();

  const progress = setup.data?.progress ?? null;
  const ended = !!progress && !setupRunning(progress);
  const [dismissed, dismiss] = useDismissed(
    ended && progress ? `setup:${server.uuid}:${progress.started.toISOString()}` : null,
  );

  if (!setup.data) return null;

  if (setup.data.pending) {
    if (!canSetUp) return null;

    return (
      <Notification
        className='theme-callout mb-6'
        title={t(`${base}.callout.heading`, {})}
        data-tour='setup' // the welcome tour's target (docs/features/welcome-tour.md)
      >
        <Stack gap='sm' align='flex-start'>
          <span>{t(`${base}.callout.body`, {})}</span>
          <NavLink to={`/server/${server.uuidShort}${SETUP_PATH}`}>
            <Button size='xs'>{t(`${base}.callout.action`, {})}</Button>
          </NavLink>
        </Stack>
      </Notification>
    );
  }

  if (!progress) return null;

  if (setupRunning(progress)) {
    return (
      <Notification className='theme-callout mb-6' loading title={t(`${base}.progress.title`, {})}>
        <SetupProgress onboarding={setup.data} />
      </Notification>
    );
  }

  if (dismissed) return null;

  const failed = progress.step === 'failed';

  return (
    <Notification
      className='theme-callout mb-6'
      color={failed ? 'red' : 'green'}
      title={failed ? t(`${base}.progress.failedHeading`, {}) : t(`${base}.progress.doneHeading`, {})}
      withCloseButton
      closeButtonProps={{ 'aria-label': t('common.button.close', {}) }}
      onClose={dismiss}
    >
      <Stack gap='xs'>
        <span>
          {failed ? t(`${base}.progress.failed`, { error: progress.error ?? '' }) : t(`${base}.progress.done`, {})}
        </span>
        {!failed && progress.error && <span>{t(`${base}.progress.problems`, { problems: progress.error })}</span>}
        {progress.skipped.length > 0 && (
          <span>{t(`${base}.progress.skipped`, { names: progress.skipped.join(', ') })}</span>
        )}
      </Stack>
    </Notification>
  );
}
