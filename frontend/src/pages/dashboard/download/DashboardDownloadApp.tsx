// plugged.host: the desktop app's download page, Windows and Linux (docs/features/app.md)
import { faLinux, faWindows } from '@fortawesome/free-brands-svg-icons';
import { faCheck } from '@fortawesome/free-solid-svg-icons';
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

export interface AppDownload {
  platform: 'windows' | 'linux' | 'linux-deb';
  filename: string;
  size: number;
}

export interface AppInfo {
  enabled: boolean;
  version: string;
  downloads: AppDownload[];
  releaseNotes: string | null;
}

const onLinux = () => /linux/i.test(navigator.userAgent) && !/android/i.test(navigator.userAgent);

export default function DashboardDownloadApp() {
  const { t } = useTranslations();
  const appName = useGlobalStore((state) => state.settings.app.name);
  const noprofit = useGlobalStore((state) => state.settings.noprofit.enabled);
  const [downloading, setDownloading] = useState<string | null>(null);
  const page = 'pages.account.downloadApp';
  const info = useResource({
    queryKey: ['user', 'app-info'],
    queryFn: async (): Promise<AppInfo> => {
      const { data } = await axiosInstance.get('/api/client/app');
      const downloads: AppDownload[] = data.downloads ?? [];
      return {
        enabled: downloads.length > 0,
        version: data.app.version,
        downloads,
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
                  <div className='flex flex-wrap gap-3'>
                    {[...app.downloads]
                      .sort(
                        (a, b) =>
                          Number(b.platform.startsWith('linux') === onLinux()) -
                          Number(a.platform.startsWith('linux') === onLinux()),
                      )
                      .map((download, index) => (
                        <Button
                          key={download.platform}
                          size='lg'
                          variant={index === 0 ? 'filled' : 'default'}
                          leftSection={<FontAwesomeIcon icon={download.platform === 'windows' ? faWindows : faLinux} />}
                          loading={downloading === download.platform}
                          onClick={() => {
                            setDownloading(download.platform);
                            window.location.assign(`/api/client/app/download?platform=${download.platform}`);
                            window.setTimeout(() => setDownloading(null), 4000);
                          }}
                        >
                          {t(
                            download.platform === 'windows'
                              ? `${page}.download`
                              : download.platform === 'linux'
                                ? `${page}.downloadLinux`
                                : `${page}.downloadDeb`,
                            {},
                          )}
                        </Button>
                      ))}
                  </div>
                </div>

                <p className='theme-eco-muted max-w-3xl'>{t(`${page}.intro`, { app: appName })}</p>
                <p className='theme-eco-muted max-w-3xl'>{t(`${page}.betaNotice`, {})}</p>
                {app.downloads.some((download) => download.platform === 'windows') && (
                  <p className='theme-eco-small max-w-3xl'>{t(`${page}.unsigned`, {})}</p>
                )}
                {app.downloads.some((download) => download.platform !== 'windows') && (
                  <p className='theme-eco-small max-w-3xl'>{t(`${page}.linuxNote`, {})}</p>
                )}

                <div className='flex flex-col gap-4'>
                  {app.downloads.map((download) => (
                    <div key={download.platform} className='flex flex-wrap gap-10'>
                      <div className='flex flex-col gap-1'>
                        <span className='theme-eco-small'>{t(`${page}.platform`, {})}</span>
                        <span className='font-mono text-sm'>
                          {download.platform === 'windows'
                            ? 'Windows 10 / 11'
                            : download.platform === 'linux'
                              ? 'Linux x86_64 (AppImage)'
                              : 'Debian / Ubuntu x86_64 (.deb)'}
                        </span>
                      </div>
                      <div className='flex flex-col gap-1'>
                        <span className='theme-eco-small'>{t(`${page}.size`, {})}</span>
                        <span className='font-mono text-sm'>{bytesToString(download.size, 1)}</span>
                      </div>
                      <div className='flex flex-col gap-1'>
                        <span className='theme-eco-small'>{t(`${page}.installer`, {})}</span>
                        <span className='font-mono text-sm'>{download.filename}</span>
                      </div>
                    </div>
                  ))}
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
