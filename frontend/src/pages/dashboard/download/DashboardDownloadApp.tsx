import { faCheck, faDownload } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useState } from 'react';
import { axiosInstance } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { bytesToString } from '@/lib/format/size.ts';
import { useResource } from '@/plugins/resource/useResource.ts';

export interface AppInfo {
  enabled: boolean;
  version: string;
  filename: string;
  size: number;
  releaseNotes: string | null;
}

export default function DashboardDownloadApp() {
  const [downloading, setDownloading] = useState(false);
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
    <AccountContentContainer title='Download app' subtitle='plugged.host for Windows'>
      <ResourceView resource={info}>
        {(app) =>
          app.enabled ? (
            <div className='flex flex-col gap-6'>
              <section className='theme-eco-card'>
                <div className='flex flex-wrap items-center gap-5'>
                  <img
                    src='/plugged.host.png'
                    alt='plugged.host'
                    className='size-16 shrink-0 rounded-2xl'
                  />
                  <div className='flex min-w-0 flex-1 flex-col gap-1'>
                    <h2 className='theme-eco-heading'>plugged.host for Windows</h2>
                    <p className='theme-eco-muted'>
                      Version {app.version} · made by noprofit.space
                    </p>
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
                    Download for Windows
                  </Button>
                </div>

                <p className='theme-eco-muted max-w-3xl'>
                  The native client for plugged.host. It talks to the panel directly, keeps the console live and opens
                  in a fraction of the time a browser does.
                </p>

                <div className='flex flex-wrap gap-10'>
                  <div className='flex flex-col gap-1'>
                    <span className='theme-eco-small'>Platform</span>
                    <span className='font-mono text-sm'>Windows 10 / 11</span>
                  </div>
                  <div className='flex flex-col gap-1'>
                    <span className='theme-eco-small'>Size</span>
                    <span className='font-mono text-sm'>{bytesToString(app.size, 1)}</span>
                  </div>
                  <div className='flex flex-col gap-1'>
                    <span className='theme-eco-small'>Installer</span>
                    <span className='font-mono text-sm'>{app.filename}</span>
                  </div>
                </div>
              </section>

              <section className='theme-eco-card'>
                <h3 className='theme-eco-heading'>What you get</h3>
                <ul className='flex flex-col gap-2'>
                  {[
                    'A real native window, not a wrapped website',
                    'Live console and resource charts',
                    'Files, backups, schedules and settings in the same place',
                    'Signs in with noprofit.space',
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
                  <h3 className='theme-eco-heading'>Release notes</h3>
                  <p className='theme-eco-muted whitespace-pre-wrap'>{app.releaseNotes}</p>
                </section>
              )}
            </div>
          ) : (
            <section className='theme-eco-card'>
              <h3 className='theme-eco-heading'>Not available on this panel</h3>
              <p className='theme-eco-muted'>The desktop app is only offered on the dev panel for now.</p>
            </section>
          )
        }
      </ResourceView>
    </AccountContentContainer>
  );
}
