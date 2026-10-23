// plugged.host: asks visitors with an ad blocker to allow ads (docs/features/ads.md#ad-blockers)
import { useEffect, useState } from 'react';
import Button from '@/elements/buttons/Button.tsx';
import Notification from '@/elements/feedback/Notification.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { dismiss, dismissed } from '@/lib/ads.ts';
import { type AdArea, useAds } from '@/plugins/useAds.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

const DISMISSED = 'ads:adblock-dismissed';

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** A bait element ad blockers hide by its class names. */
async function baitHidden(): Promise<boolean> {
  const bait = document.createElement('div');
  bait.className = 'adsbox ad-banner ad-placement ad-slot pub_300x250 pub_728x90 text-ad textAd sponsor-ad';
  bait.setAttribute('aria-hidden', 'true');
  bait.style.cssText = 'position:absolute;inset-inline-start:-9999px;top:-9999px;width:300px;height:250px;';
  bait.innerHTML = '&nbsp;';
  document.body.appendChild(bait);

  const hidden = () => {
    if (!document.body.contains(bait) || bait.offsetHeight === 0) return true;
    const style = window.getComputedStyle(bait);
    return style.display === 'none' || style.visibility === 'hidden';
  };

  try {
    await sleep(100);
    if (hidden()) return true;
    // some blockers apply their cosmetic rules late
    await sleep(2500);
    return hidden();
  } finally {
    bait.remove();
  }
}

/** Whether the ad host refuses a request twice (a blocker fails it at once; a timeout does not count). */
async function hostRefused(host: string): Promise<boolean> {
  const attempt = async () => {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 5000);
    try {
      await fetch(`https://${host}/`, {
        mode: 'no-cors',
        cache: 'no-store',
        credentials: 'omit',
        signal: controller.signal,
      });
      return false;
    } catch (error) {
      return !(error instanceof DOMException && error.name === 'AbortError');
    } finally {
      clearTimeout(timeout);
    }
  };

  return (await attempt()) && (await attempt());
}

let detection: Promise<boolean> | null = null;
let detected: boolean | null = null;

/** Whether an ad blocker runs; detected once per page load. */
export function useAdBlocked(area: AdArea): boolean | null {
  const { settings, shown } = useAds(area);
  const [blocked, setBlocked] = useState<boolean | null>(detected);

  useEffect(() => {
    if (!shown || !settings.adblockEnabled) return;

    if (!detection) {
      const host = settings.zones[0]?.host ?? null;
      detection = Promise.all([baitHidden(), settings.adblockProbe && host ? hostRefused(host) : false]).then(
        ([bait, probe]) => {
          detected = bait || probe;
          return detected;
        },
      );
    }

    let current = true;
    detection.then((value) => current && setBlocked(value));
    return () => {
      current = false;
    };
  }, [shown, settings?.adblockEnabled]);

  return blocked;
}

/** The notice, modal or block the settings ask for, once an ad blocker is found. */
export default function AdBlockNotice({ area, console: onConsole }: { area: AdArea; console: boolean }) {
  const { t } = useTranslations();
  const { settings, shown } = useAds(area);
  const blocked = useAdBlocked(area);
  const [closed, setClosed] = useState(() => dismissed(DISMISSED, settings?.adblockDismissHours ?? 0));
  const [grace, setGrace] = useState(settings?.adblockGrace ?? 0);

  const mode = settings?.adblockMode ?? 'notice';
  const visible = shown && settings.adblockEnabled && blocked === true && !(onConsole && settings.adblockSkipConsole);

  useEffect(() => {
    if (!visible || mode !== 'modal' || grace <= 0) return;

    const interval = setInterval(() => setGrace((grace) => Math.max(0, grace - 1)), 1000);
    return () => clearInterval(interval);
  }, [visible, mode, grace > 0]);

  if (!visible || (mode !== 'block' && closed)) return null;

  const message = settings.adblockMessage ?? t('elements.ads.adblock.message', {});
  const close = () => {
    dismiss(DISMISSED);
    setClosed(true);
  };

  if (mode === 'notice') {
    return (
      <div className='theme-main theme-main-banner'>
        <Notification
          className='theme-callout'
          color='yellow'
          title={t('elements.ads.adblock.title', {})}
          withCloseButton
          closeButtonProps={{ 'aria-label': t('common.button.close', {}), title: t('common.button.close', {}) }}
          onClose={close}
        >
          {message}
        </Notification>
      </div>
    );
  }

  return (
    <Modal
      opened
      onClose={mode === 'modal' && grace === 0 ? close : () => undefined}
      withCloseButton={false}
      closeOnClickOutside={false}
      closeOnEscape={mode === 'modal' && grace === 0}
      title={t('elements.ads.adblock.title', {})}
    >
      <Text size='sm'>{message}</Text>
      <ModalFooter>
        <Button onClick={() => window.location.reload()}>{t('elements.ads.adblock.button.reload', {})}</Button>
        {mode === 'modal' && (
          <Button variant='default' disabled={grace > 0} onClick={close}>
            {grace > 0 ? t('elements.ads.adblock.button.closeIn', { seconds: grace }) : t('common.button.close', {})}
          </Button>
        )}
      </ModalFooter>
    </Modal>
  );
}
