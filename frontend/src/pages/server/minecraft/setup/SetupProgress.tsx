// plugged.host: how far a setup got: its step, and the modpack's own progress while it installs one (docs/features/minecraft.md)
import { z } from 'zod';
import Progress from '@/elements/feedback/Progress.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { minecraftOnboardingSchema } from '@/lib/schemas/server/minecraft.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import ModpackProgress from '../content/ModpackProgress.tsx';
import { modpackInstalling } from '../content/useModpacks.ts';

const base = 'pages.server.minecraft.setup';

type Onboarding = z.infer<typeof minecraftOnboardingSchema>;
type RunningStep = Exclude<NonNullable<Onboarding['progress']>['step'], 'done' | 'failed'>;

export default function SetupProgress({ onboarding }: { onboarding: Onboarding }) {
  const { t } = useTranslations();
  const installing = useServerStore((state) => state.server.status === 'installing');
  const progress = onboarding.progress;
  if (!progress) return null;

  const step = progress.step as RunningStep;
  const modpack = step === 'modpack' && modpackInstalling(onboarding.modpack) ? onboarding.modpack : null;
  // the pack is downloaded and read; it goes on the server when the install that runs is over (a
  // server created without its install has none to wait for)
  const packWaits =
    installing &&
    !!modpack &&
    modpack.step === 'download' &&
    modpack.totalBytes > 0 &&
    modpack.bytes >= modpack.totalBytes;

  return (
    <Stack gap='xs'>
      {modpack && packWaits ? (
        <>
          <span>{modpack.name}</span>
          <span>{t(`${base}.progress.waiting`, {})}</span>
          <Progress value={NaN} indeterminate />
        </>
      ) : modpack ? (
        <>
          <span>{modpack.name}</span>
          <ModpackProgress install={modpack} />
        </>
      ) : (
        <>
          <span>{t(`${base}.progress.${step}`, {})}</span>
          {step !== 'ready' && <Progress value={NaN} indeterminate />}
        </>
      )}
      {progress.fatal ? (
        <span>{t(`${base}.progress.failed`, { error: progress.fatal })}</span>
      ) : (
        progress.problems.length > 0 && (
          <span>{t(`${base}.progress.problems`, { problems: progress.problems.join('; ') })}</span>
        )
      )}
    </Stack>
  );
}
