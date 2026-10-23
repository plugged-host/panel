// plugged.host: Installed | Browse on the Plugins & Mods page (docs/features/minecraft.md)
import { faMagnifyingGlass, faPuzzlePiece } from '@fortawesome/free-solid-svg-icons';
import SubNavigation from '@/elements/navigation/SubNavigation.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

export default function ContentSubNavigation() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);

  const base = `/server/${server.uuidShort}/minecraft/content`;

  return (
    <SubNavigation
      baseUrl={base}
      items={[
        {
          name: t('pages.server.minecraft.content.installed.title', {}),
          icon: faPuzzlePiece,
          link: base,
        },
        {
          name: t('pages.server.minecraft.content.browse.title', {}),
          icon: faMagnifyingGlass,
          link: `${base}/browse`,
        },
      ]}
    />
  );
}
