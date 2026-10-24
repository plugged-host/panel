// plugged.host: Installed | Browse | Modpacks on the Plugins & Mods page (docs/features/minecraft.md)
import { faBoxesStacked, faMagnifyingGlass, faPuzzlePiece } from '@fortawesome/free-solid-svg-icons';
import SubNavigation from '@/elements/navigation/SubNavigation.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { useContentOptions } from './useContentOptions.ts';

export default function ContentSubNavigation() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);
  const options = useContentOptions();

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
        ...(options.data?.modpacks
          ? [
              {
                name: t('pages.server.minecraft.modpacks.title', {}),
                icon: faBoxesStacked,
                link: `${base}/modpacks`,
              },
            ]
          : []),
      ]}
    />
  );
}
