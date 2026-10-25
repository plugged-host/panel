// plugged.host: "Installed on this server", the Pelican panel's installed list (installed.blade.php, docs/features/minecraft.md)
import { useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import getInstalledContent from '@/api/server/minecraft/getInstalledContent.ts';
import Spinner from '@/elements/feedback/Spinner.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import InstalledRow from './InstalledRow.tsx';
import { ContentOptions } from './useContentOptions.ts';

// the list identifies a few jars per request; ask again until it is done
const IDENTIFY_AGAIN_MS = 1500;

export default function InstalledSection({ options }: { options: ContentOptions }) {
  const { t } = useTranslations();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const base = 'pages.server.minecraft.content.installed';

  const installed = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.installed(),
    queryFn: () => getInstalledContent(server.uuid),
    keepPrevious: true,
  });
  const jars = installed.data?.installed ?? [];
  const pending = installed.data?.pending ?? 0;

  useEffect(() => {
    if (!pending) return;

    const timeout = setTimeout(() => installed.refetch(), IDENTIFY_AGAIN_MS);
    return () => clearTimeout(timeout);
  }, [installed.data]);

  const refresh = () =>
    queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.installed() });

  return (
    <section>
      <div className='theme-jar-head'>
        <h3 className='theme-eco-heading'>
          {t(`${base}.heading`, {})}
          {jars.length > 0 && <span className='theme-jar-count'>{jars.length}</span>}
        </h3>
        {pending > 0 && (
          <span className='theme-jar-progress'>
            <Spinner size={14} />
            {t(`${base}.identifying`, { count: pending })}
          </span>
        )}
      </div>

      <ResourceView resource={installed}>
        {(data) =>
          data.installed.length === 0 ? (
            <div className='theme-jar-empty'>{t(`${base}.empty`, {})}</div>
          ) : (
            <div className='theme-jar-list'>
              {data.installed.map((jar) => (
                <InstalledRow key={jar.file} jar={jar} directory={options.directory ?? ''} onChanged={refresh} />
              ))}
            </div>
          )
        }
      </ResourceView>
    </section>
  );
}
