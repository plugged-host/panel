// plugged.host: the Windows app's download page (docs/features/app.md)
import { faCheck, faDownload } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useState } from 'react';
import { axiosInstance } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { bytesToString } from '@/lib/format/size.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

export interface AppInfo {
  enabled: boolean;
  version: string;
  filename: string;
  size: number;
  releaseNotes: string | null;
}

export default function DashboardDownloadApp() {
  const { t } = useTranslations();
  const appName = useGlobalStore((state) => state.settings.app.name);
  const noprofit = useGlobalStore((state) => state.settings.noprofit.enabled);
  const [downloading, setDownloading] = useState(false);
  const page = 'pages.account.downloadApp';
  const info = useResource({
    queryKey: ['user', 'app-info'],
    queryFn: async (): Promise<AppInfo> => {
      const { data } = await axiosInstance.get('/api/client/app');
      return {
        enabled: data.app.enabled,
        version: data.app.version,
        filename: data.app.filename,
        size: data.app.size,
        releaseNotes: data.app.release_notes ?? null,
      };
    },
  });

  return (
    <AccountContentContainer title={t(`${page}.title`, {})} subtitle={t(`${page}.subtitle`, { app: appName })}>
      <ResourceView resource={info}>
        {(app) =>
          app.enabled ? (
            <div className='flex flex-col gap-6'>
              <section className='theme-eco-card'>
                <div className='flex flex-wrap items-center gap-5'>
                  <img src='/plugged.host.png' alt='plugged.host' className='size-16 shrink-0 rounded-2xl' />
                  <div className='flex min-w-0 flex-1 flex-col gap-1'>
                    <h2 className='theme-eco-heading flex items-center gap-2'>
                      {t(`${page}.subtitle`, { app: appName })}
                      <Badge size='sm' variant='light' color='blue'>
                        {t(`${page}.beta`, {})}
                      </Badge>
                    </h2>
                    <p className='theme-eco-muted'>{t(`${page}.version`, { version: app.version })}</p>
                  </div>
                  <Button
                    size='lg'
                    leftSection={<FontAwesomeIcon icon={faDownload} />}
                    loading={downloading}
                    onClick={() => {
                      setDownloading(true);
                      window.location.assign('/api/client/app/download');
                      window.setTimeout(() => setDownloading(false), 4000);
                    }}
                  >
                    {t(`${page}.download`, {})}
                  </Button>
                </div>

                <p className='theme-eco-muted max-w-3xl'>{t(`${page}.intro`, { app: appName })}</p>
                <p className='theme-eco-muted max-w-3xl'>{t(`${page}.betaNotice`, {})}</p>
                <p className='theme-eco-small max-w-3xl'>{t(`${page}.unsigned`, {})}</p>

                <div className='flex flex-wrap gap-10'>
                  <div className='flex flex-col gap-1'>
                    <span className='theme-eco-small'>{t(`${page}.platform`, {})}</span>
                    <span className='font-mono text-sm'>Windows 10 / 11</span>
                  </div>
                  <div className='flex flex-col gap-1'>
                    <span className='theme-eco-small'>{t(`${page}.size`, {})}</span>
                    <span className='font-mono text-sm'>{bytesToString(app.size, 1)}</span>
                  </div>
                  <div className='flex flex-col gap-1'>
                    <span className='theme-eco-small'>{t(`${page}.installer`, {})}</span>
                    <span className='font-mono text-sm'>{app.filename}</span>
                  </div>
                </div>
              </section>

              <section className='theme-eco-card'>
                <h3 className='theme-eco-heading'>{t(`${page}.featuresTitle`, {})}</h3>
                <ul className='flex flex-col gap-2'>
                  {[
                    t(`${page}.featureNative`, {}),
                    t(`${page}.featureConsole`, {}),
                    t(`${page}.featureManage`, {}),
                    ...(noprofit ? [t(`${page}.featureSignIn`, {})] : []),
                  ].map((line) => (
                    <li key={line} className='flex items-center gap-2 text-sm'>
                      <FontAwesomeIcon icon={faCheck} className='text-green-500' />
                      {line}
                    </li>
                  ))}
                </ul>
              </section>

              {app.releaseNotes && (
                <section className='theme-eco-card'>
                  <h3 className='theme-eco-heading'>{t(`${page}.releaseNotes`, {})}</h3>
                  <p className='theme-eco-muted whitespace-pre-wrap'>{app.releaseNotes}</p>
                </section>
              )}
            </div>
          ) : (
            <section className='theme-eco-card'>
              <h3 className='theme-eco-heading'>{t(`${page}.unavailableTitle`, {})}</h3>
              <p className='theme-eco-muted'>{t(`${page}.unavailableText`, {})}</p>
            </section>
          )
        }
      </ResourceView>
    </AccountContentContainer>
  );
}
