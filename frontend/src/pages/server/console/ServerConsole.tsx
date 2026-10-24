import classNames from 'classnames';
import { useEffect, useRef, useState } from 'react';
import { ServerCan } from '@/elements/Can.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import { useNavbarPageHeader } from '@/elements/containers/useNavbarPageHeader.ts';
import Group from '@/elements/layout/Group.tsx';
import LogCheckBanner from '@/pages/server/minecraft/LogCheckBanner.tsx'; // plugged.host
import { useVisualViewportBottomInset } from '@/plugins/viewport/useVisualViewport.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import ServerOverview from './stats/ServerOverview.tsx'; // plugged.host: instead of ServerDetails
import ServerPowerControls from './stats/ServerPowerControls.tsx';
import ServerStats from './stats/ServerStats.tsx';
import Console from './terminal/Console.tsx';
import WelcomeTourServer from './WelcomeTourServer.tsx'; // plugged.host

export default function ServerConsole() {
  const { t } = useTranslations();
  const keyboardInset = useVisualViewportBottomInset();
  const title = t('pages.server.console.title', {}); // plugged.host: "Console", as Pelican's page title
  const titleHoisted = useNavbarPageHeader({ title });

  const infobarSentinelRef = useRef<HTMLDivElement>(null);
  const [infobarStuck, setInfobarStuck] = useState(false);

  useEffect(() => {
    if (!titleHoisted || !infobarSentinelRef.current) return;

    const observer = new IntersectionObserver(([entry]) =>
      setInfobarStuck(!entry.isIntersecting && entry.boundingClientRect.top < 0),
    );
    observer.observe(infobarSentinelRef.current);

    return () => observer.disconnect();
  }, [titleHoisted]);

  return (
    <ServerContentContainer
      title={title}
      hideTitleComponent
      registry={window.extensionContext.extensionRegistry.pages.server.console.container}
    >
      {/* plugged.host: what Pelican renders at the page's start (PAGE_START), above the title row */}
      <LogCheckBanner />
      <div ref={infobarSentinelRef} />
      <div
        id='console-infobar'
        className={classNames(
          'sticky z-20 bg-(--theme-bg-top) mb-6 py-2 -mx-4 md:-mx-6 lg:-mx-8 pr-4 md:pr-6 lg:px-8 transition-[padding] duration-150',
          titleHoisted ? 'top-3 lg:top-0 shadow-[0_-0.75rem_0_var(--theme-bg-top)] lg:shadow-none' : 'top-0',
          titleHoisted && infobarStuck ? 'pl-20' : 'pl-4 md:pl-6',
        )}
      >
        <Group justify='space-between' className={classNames({ 'min-h-12 lg:min-h-0': titleHoisted })}>
          <div className={classNames('flex-col min-w-0', titleHoisted ? 'hidden lg:flex' : 'flex')}>
            <h1 className='theme-header-heading'>{title}</h1>
          </div>
          <ServerCan action={['control.start', 'control.stop', 'control.restart']} matchAny>
            <ServerPowerControls />
          </ServerCan>
        </Group>
      </div>
      {/* plugged.host: the Pelican console's order: the stat blocks, the console the full width under
          them, then the CPU, memory and network charts, 1.5rem apart. */}
      <div
        className='mb-6 min-w-0'
        data-tour='details' // plugged.host: the welcome tour's targets
      >
        <ServerOverview />
      </div>
      <div
        className='mb-6 flex flex-col h-[60vh]'
        data-tour='console' // plugged.host: the welcome tour's targets
        style={
          keyboardInset > 0 ? { height: `max(8rem, min(60vh, calc(100dvh - ${keyboardInset}px - 7rem)))` } : undefined
        }
      >
        <Console />
      </div>
      <ServerStats />
      <WelcomeTourServer /> {/* plugged.host */}
    </ServerContentContainer>
  );
}
