// plugged.host: the welcome tour's overlay: it dims the page, leaves the element a step is about
// lit, and puts the step's text next to it (docs/features/welcome-tour.md). A step whose element
// is not on screen (a narrow window hides the sidebar) shows in the middle instead.
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import Button from '@/elements/buttons/Button.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export interface TourStep {
  key: string;
  /** CSS selectors of the element to light, the first one on screen wins. */
  targets: string[];
  heading: string;
  body: string;
  bullets?: string[];
}

interface Props {
  steps: TourStep[];
  /** 1-based. */
  step: number;
  onStep: (step: number) => void;
  /** The last step's button. */
  finishLabel: string;
  onFinish: () => void;
  /** The way out before the end ("Skip tour", "Not now"). */
  closeLabel: string;
  onClose: () => void;
}

interface Rect {
  top: number;
  left: number;
  width: number;
  height: number;
}

const PAD = 8;
const GAP = 16;
const CARD_WIDTH = 360;

function findTarget(selectors: string[]): HTMLElement | null {
  for (const selector of selectors) {
    for (const element of document.querySelectorAll<HTMLElement>(selector)) {
      const rect = element.getBoundingClientRect();
      if (rect.width > 0 && rect.height > 0) {
        return element;
      }
    }
  }

  return null;
}

function measure(element: HTMLElement | null): Rect | null {
  if (!element) return null;

  const rect = element.getBoundingClientRect();
  if (rect.width === 0 || rect.height === 0) return null;

  // the lit area stays inside the window, so a tall element still leaves room for the card
  const top = Math.max(rect.top - PAD, PAD);
  const left = Math.max(rect.left - PAD, PAD);
  const bottom = Math.min(rect.bottom + PAD, window.innerHeight - PAD);
  const right = Math.min(rect.right + PAD, window.innerWidth - PAD);

  return { top, left, width: Math.max(right - left, 0), height: Math.max(bottom - top, 0) };
}

function sameRect(a: Rect | null, b: Rect | null): boolean {
  if (!a || !b) return a === b;

  return a.top === b.top && a.left === b.left && a.width === b.width && a.height === b.height;
}

/** Where the card goes: beside a narrow element on the left (the sidebar), else under or over it. */
function placeCard(hole: Rect | null, cardHeight: number): { top: number; left: number } | null {
  const width = Math.min(CARD_WIDTH, window.innerWidth - 2 * GAP);
  if (!hole || window.innerWidth < 768) return null;

  const clampTop = (top: number) => Math.min(Math.max(top, GAP), window.innerHeight - cardHeight - GAP);
  const clampLeft = (left: number) => Math.min(Math.max(left, GAP), window.innerWidth - width - GAP);

  if (hole.left + hole.width < window.innerWidth * 0.4 && hole.left + hole.width + GAP + width < window.innerWidth) {
    return { top: clampTop(hole.top), left: hole.left + hole.width + GAP };
  }
  if (hole.top + hole.height + GAP + cardHeight < window.innerHeight) {
    return { top: hole.top + hole.height + GAP, left: clampLeft(hole.left) };
  }
  if (hole.top - GAP - cardHeight > 0) {
    return { top: hole.top - GAP - cardHeight, left: clampLeft(hole.left) };
  }

  return { top: clampTop(hole.top), left: clampLeft(hole.left + hole.width - width) };
}

export default function WelcomeTour({ steps, step, onStep, finishLabel, onFinish, closeLabel, onClose }: Props) {
  const { t } = useTranslations();
  const index = Math.min(Math.max(step, 1), steps.length) - 1;
  const current = steps[index];
  const last = index === steps.length - 1;

  const [hole, setHole] = useState<Rect | null>(null);
  const [cardHeight, setCardHeight] = useState(240);
  const cardRef = useRef<HTMLDivElement>(null);

  // the steps are rebuilt on every render of the caller: the element is followed by its selectors
  const targetsKey = current ? current.targets.join('\n') : '';

  // follow the element: it may render late (a list still loading), move on scroll or resize
  useLayoutEffect(() => {
    const targets = targetsKey ? targetsKey.split('\n') : [];

    findTarget(targets)?.scrollIntoView({ block: 'nearest', inline: 'nearest' });

    let frame = 0;
    const update = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const next = measure(findTarget(targets));
        setHole((previous) => (sameRect(previous, next) ? previous : next));
      });
    };

    update();
    window.addEventListener('resize', update);
    window.addEventListener('scroll', update, true);
    const interval = setInterval(update, 500);

    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener('resize', update);
      window.removeEventListener('scroll', update, true);
      clearInterval(interval);
    };
  }, [targetsKey]);

  useLayoutEffect(() => {
    const height = cardRef.current?.offsetHeight;
    if (height && height !== cardHeight) setCardHeight(height);
  });

  // the step's first button takes the focus, so Enter goes on
  useEffect(() => {
    cardRef.current?.querySelector<HTMLButtonElement>('.theme-tour-actions button')?.focus();
  }, [index]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);

    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  if (!current) return null;

  const position = placeCard(hole, cardHeight);
  const scrims: Rect[] = hole
    ? [
        { top: 0, left: 0, width: window.innerWidth, height: hole.top },
        { top: hole.top + hole.height, left: 0, width: window.innerWidth, height: window.innerHeight },
        { top: hole.top, left: 0, width: hole.left, height: hole.height },
        { top: hole.top, left: hole.left + hole.width, width: window.innerWidth, height: hole.height },
      ]
    : [{ top: 0, left: 0, width: window.innerWidth, height: window.innerHeight }];

  const setBox = (element: HTMLElement | null, rect: Rect) => {
    if (!element) return;
    element.style.setProperty('top', `${rect.top}px`);
    element.style.setProperty('left', `${rect.left}px`);
    element.style.setProperty('width', `${rect.width}px`);
    element.style.setProperty('height', `${rect.height}px`);
  };

  return createPortal(
    <div className='theme-tour'>
      {scrims.map((rect, i) => (
        <div key={i} className='theme-tour-scrim' ref={(element) => setBox(element, rect)} />
      ))}
      {hole && <div className='theme-tour-ring' ref={(element) => setBox(element, hole)} />}

      <div
        ref={(element) => {
          cardRef.current = element;
          if (!element) return;
          if (position) {
            element.style.setProperty('top', `${position.top}px`);
            element.style.setProperty('left', `${position.left}px`);
          } else {
            element.style.removeProperty('top');
            element.style.removeProperty('left');
          }
        }}
        className='theme-tour-card'
        data-placed={position ? true : undefined}
        role='dialog'
        aria-modal='true'
        aria-labelledby='theme-tour-heading'
      >
        <p className='theme-tour-progress'>
          {t('elements.welcomeTour.progress', { step: index + 1, total: steps.length })}
        </p>
        <h2 id='theme-tour-heading' className='theme-tour-heading'>
          {current.heading}
        </h2>
        <p className='theme-tour-body'>{current.body}</p>
        {current.bullets && current.bullets.length > 0 && (
          <ul className='theme-tour-bullets'>
            {current.bullets.map((bullet) => (
              <li key={bullet}>{bullet}</li>
            ))}
          </ul>
        )}

        <div className='theme-tour-actions'>
          <Button onClick={() => (last ? onFinish() : onStep(index + 2))}>
            {last ? finishLabel : t('elements.welcomeTour.next', {})}
          </Button>
          {index > 0 && (
            <Button variant='default' onClick={() => onStep(index)}>
              {t('elements.welcomeTour.back', {})}
            </Button>
          )}
          {!last && (
            <Button variant='subtle' color='gray' className='theme-tour-close' onClick={onClose}>
              {closeLabel}
            </Button>
          )}
        </div>
      </div>
    </div>,
    document.body,
  );
}
