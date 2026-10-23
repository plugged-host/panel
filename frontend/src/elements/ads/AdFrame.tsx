// plugged.host: one ad unit in its sandboxed frame (docs/features/ads.md#isolation)
import classNames from 'classnames';
import { useEffect, useRef, useState } from 'react';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

/** Seconds a unit has to report an ad before it folds away. */
const FILL_WAIT = 6;

interface Props {
  src: string;
  sandbox: string;
  width: number | null;
  height: number;
  refreshSeconds: number;
}

/**
 * The frame's document reports whether it holds an ad (`{adUnit, filled}`); a unit without one
 * folds, so no empty box stays. A filled unit reloads after `refreshSeconds` in view.
 */
export default function AdFrame({ src, sandbox, width, height, refreshSeconds }: Props) {
  const { t } = useTranslations();
  const frame = useRef<HTMLIFrameElement>(null);
  const box = useRef<HTMLDivElement>(null);
  const [round, setRound] = useState(0);
  const [filled, setFilled] = useState<boolean | null>(null);

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      if (event.source !== frame.current?.contentWindow) return;
      if (event.data && typeof event.data === 'object' && event.data.adUnit === true) {
        setFilled(event.data.filled === true);
      }
    };

    window.addEventListener('message', onMessage);
    return () => window.removeEventListener('message', onMessage);
  }, []);

  useEffect(() => {
    setFilled(null);
    const timeout = setTimeout(() => setFilled((current) => current ?? false), FILL_WAIT * 1000);
    return () => clearTimeout(timeout);
  }, [src, round]);

  useEffect(() => {
    if (!refreshSeconds || filled !== true || !box.current) return;

    let visible = false;
    let seen = 0;
    const observer = new IntersectionObserver(
      ([entry]) => {
        visible = entry.isIntersecting && entry.intersectionRatio >= 0.5;
      },
      { threshold: [0, 0.5, 1] },
    );
    observer.observe(box.current);

    const interval = setInterval(() => {
      if (!visible || document.visibilityState !== 'visible') return;
      seen += 1;
      if (seen >= refreshSeconds) {
        seen = 0;
        setRound((round) => round + 1);
      }
    }, 1000);

    return () => {
      observer.disconnect();
      clearInterval(interval);
    };
  }, [refreshSeconds, filled]);

  return (
    <div
      ref={box}
      className={classNames('theme-ad-frame', filled === false && 'theme-ad-folded')}
      style={{ width: width ?? '100%', height: filled === false ? 0 : height }}
    >
      <iframe
        key={round}
        ref={frame}
        src={src}
        sandbox={sandbox}
        title={t('elements.ads.frameTitle', {})}
        width={width ?? undefined}
        height={height}
        loading='lazy'
        referrerPolicy='strict-origin-when-cross-origin'
      />
    </div>
  );
}
