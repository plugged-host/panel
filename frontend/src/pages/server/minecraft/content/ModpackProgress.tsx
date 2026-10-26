// plugged.host: how far a modpack install got, as a bar: the step, files of the total and their size when the pack says (docs/features/minecraft.md)
import Progress from '@/elements/feedback/Progress.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { bytesProgressString } from '@/lib/format/size.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { minecraftLoaderLabels } from './labels.ts';
import { ModpackInstall } from './useModpacks.ts';

const base = 'pages.server.minecraft.modpacks';

/** The bar's share: the pack's bytes while it downloads, then the files (by size when known). */
function share(install: ModpackInstall): number {
  if (install.step === 'download') {
    return install.totalBytes > 0 ? (install.bytes / install.totalBytes) * 100 : NaN;
  }
  // the reinstall runs with no measure of its own until the first files are in
  if (install.total === 0 || (install.step !== 'files' && install.files === 0)) return NaN;
  if (install.step === 'overrides' && install.files >= install.total) return NaN;
  return install.totalBytes > 0 ? (install.bytes / install.totalBytes) * 100 : (install.files / install.total) * 100;
}

export default function ModpackProgress({ install }: { install: ModpackInstall }) {
  const { t } = useTranslations();
  const value = share(install);
  const counting = install.step !== 'download' && install.total > 0 && (install.step === 'files' || install.files > 0);
  const sized = install.step === 'download' ? install.totalBytes > 0 : counting && install.totalBytes > 0;

  return (
    <Stack gap='xs'>
      <span>
        {t(`${base}.step.${install.step}`, {})}
        {install.step === 'software' && install.loader && install.minecraft
          ? ` ${t(`${base}.state.software`, { loader: minecraftLoaderLabels[install.loader], version: install.minecraft })}`
          : ''}
        {counting ? ` ${install.files}/${install.total}` : ''}
        {sized ? ` · ${bytesProgressString(install.bytes, install.totalBytes)}` : ''}
      </span>
      <Progress value={value} indeterminate={!Number.isFinite(value)} />
    </Stack>
  );
}
