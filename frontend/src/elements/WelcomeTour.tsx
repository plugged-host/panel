// plugged.host: the welcome tour's overlay, the Pelican panel's (plugged-host-core's
// panel-tour-overlay.blade.php; docs/features/welcome-tour.md): a sheet that takes every click, so
// the page cannot be used mid-tour; a hole around the element a step is about, whose outline is
// the dim and whose border the accent ring; and the step as a modal window beside it. With nothing
// to point at (the welcome step, or an element not on screen: a narrow window hides the sidebar)
// the card is centred and wider; on phones it is a sheet on the side the element is not on.
import { useFocusTrap, useMergedRef } from '@mantine/hooks';
import classNames from 'classnames';
import { useEffect, useLayoutEffect, useRef } from 'react';
import { createPortal } from 'react-dom';
import Button from '@/elements/buttons/Button.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export interface TourStep {
  key: string;
  /** CSS selectors of the element to light, the first one on screen wins. */
  targets: string[];
  /** Light everything the winning selector finds together (the sidebar's rows), not only the first. */
  together?: boolean;
  heading: string;
  body: string;
  bullets?: string[];
}

interface Props {
  steps: TourStep[];
  /** 1-based. */
  step: number;
  onStep: (step: number) => void;
  /** Step 1's button instead of Next, the acknowledgement ("Show me around"); step 1 then has no way out. */
  acknowledgeLabel?: string;
  /** The last step's button. */
  finishLabel: string;
  onFinish: () => void;
  /** The way out before the last step ("Skip tour"), and Escape. */
  skipLabel: string;
  onSkip: () => void;
  /** The way out beside the last step's button ("Not now"); without it the last step has only its button. */
  laterLabel?: string;
  onLater?: () => void;
}

interface Hole {
  x: number;
  y: number;
  w: number;
  h: number;
}

type Mode = 'center' | 'anchored' | 'sheet-top' | 'sheet-bottom';

// between the card and the element, and between the card and the window's edge
const GAP = 16;
const MARGIN = 16;

/** The elements of the first selector that has one on screen (only the first unless `together`). */
function findTargets(selectors: string[], together: boolean, root: HTMLElement | null): HTMLElement[] {
  for (const selector of selectors) {
    let nodes: HTMLElement[];
    try {
      nodes = Array.from(document.querySelectorAll<HTMLElement>(selector));
    } catch {
      continue;
    }

    const shown = nodes.filter((node) => {
      if (root?.contains(node)) return false;
      const rect = node.getBoundingClientRect();

      return rect.width > 0 && rect.height > 0 && rect.right > 0 && rect.left < window.innerWidth;
    });
    if (shown.length > 0) {
      return together ? shown : shown.slice(0, 1);
    }
  }

  return [];
}

/** The lit box around the elements, 8px around them and inside the window; null when none shows. */
function holeAround(nodes: HTMLElement[]): Hole | null {
  if (nodes.length === 0) return null;

  let top = Infinity;
  let left = Infinity;
  let right = -Infinity;
  let bottom = -Infinity;
  for (const node of nodes) {
    const rect = node.getBoundingClientRect();
    top = Math.min(top, rect.top);
    left = Math.min(left, rect.left);
    right = Math.max(right, rect.right);
    bottom = Math.max(bottom, rect.bottom);
  }

  const x = Math.max(4, left - 8);
  const y = Math.max(4, top - 8);
  const w = Math.min(window.innerWidth - 4, right + 8) - x;
  const h = Math.min(window.innerHeight - 4, bottom + 8) - y;

  return w > 8 && h > 8 ? { x, y, w, h } : null;
}

export default function WelcomeTour({
  steps,
  step,
  onStep,
  acknowledgeLabel,
  finishLabel,
  onFinish,
  skipLabel,
  onSkip,
  laterLabel,
  onLater,
}: Props) {
  const { t } = useTranslations();
  const index = Math.min(Math.max(step, 1), steps.length) - 1;
  const current = steps[index];
  const last = index === steps.length - 1;
  const acknowledging = !!acknowledgeLabel && index === 0;

  const rootRef = useRef<HTMLDivElement>(null);
  const holeRef = useRef<HTMLDivElement>(null);
  const cardRef = useRef<HTMLElement>(null);
  const trapRef = useFocusTrap(true);
  const cardRefs = useMergedRef(cardRef, trapRef);
  // the layout the card was put in last: a new layout jumps there instead of gliding
  const placedMode = useRef<Mode | null>(null);

  // the steps are rebuilt on every render of the caller: the element is followed by its selectors
  const targetsKey = current ? current.targets.join('\n') : '';
  const together = !!current?.together;

  // Light the step's element and put the card next to it, then follow it: it may render late (a
  // list still loading) and moves on scroll and resize. Positions are written to the hole's and the
  // card's style directly, so React never puts them back.
  useLayoutEffect(() => {
    const targets = targetsKey ? targetsKey.split('\n') : [];

    const place = (card: HTMLElement, hole: Hole | null, mode: Mode) => {
      const jump = placedMode.current !== mode;
      if (jump) card.classList.add('theme-tour-jump');

      // beside the element: right, below, left, above, the first side the card fits on whole,
      // else inside the element's top end corner (a console's output starts top left)
      if (mode === 'anchored' && hole) {
        const width = window.innerWidth;
        const height = window.innerHeight;
        const w = card.offsetWidth;
        const h = card.offsetHeight;
        const sides: [number, number, boolean][] = [
          [hole.x + hole.w + GAP, hole.y, hole.x + hole.w + GAP + w <= width - MARGIN],
          [hole.x, hole.y + hole.h + GAP, hole.y + hole.h + GAP + h <= height - MARGIN],
          [hole.x - GAP - w, hole.y, hole.x - GAP - w >= MARGIN],
          [hole.x, hole.y - GAP - h, hole.y - GAP - h >= MARGIN],
        ];
        const side = sides.find((candidate) => candidate[2]);
        const x = side ? side[0] : hole.x + hole.w - w - GAP;
        const y = side ? side[1] : hole.y + GAP;

        card.style.setProperty(
          '--theme-tour-card-x',
          `${Math.round(Math.min(Math.max(x, MARGIN), width - MARGIN - w))}px`,
        );
        card.style.setProperty(
          '--theme-tour-card-y',
          `${Math.round(Math.min(Math.max(y, MARGIN), height - MARGIN - h))}px`,
        );
      }

      if (jump) {
        // flush the jump before the class comes off, or the move animates anyway
        void card.offsetWidth;
        card.classList.remove('theme-tour-jump');
      }
      placedMode.current = mode;
      card.classList.add('theme-tour-ready');
    };

    const measure = () => {
      const holeElement = holeRef.current;
      const card = cardRef.current;
      if (!holeElement || !card) return;

      const hole = holeAround(findTargets(targets, together, rootRef.current));
      const height = window.innerHeight;
      let mode: Mode = hole ? 'anchored' : 'center';

      // phones: a sheet on whichever side of the element has more room, no taller than that room
      // unless it is tiny (covering some of the element beats a card too short to read)
      if (hole && window.innerWidth < 640) {
        const above = hole.y - 32;
        const below = height - hole.y - hole.h - 32;
        mode = below >= above ? 'sheet-bottom' : 'sheet-top';
        card.style.setProperty('--theme-tour-card-max', `${Math.round(Math.max(above, below, height * 0.45))}px`);
      } else {
        card.style.removeProperty('--theme-tour-card-max');
      }

      for (const [name, value] of [
        ['x', hole?.x],
        ['y', hole?.y],
        ['w', hole?.w],
        ['h', hole?.h],
      ] as const) {
        if (value === undefined) {
          holeElement.style.removeProperty(`--theme-tour-hole-${name}`);
        } else {
          holeElement.style.setProperty(`--theme-tour-hole-${name}`, `${Math.round(value)}px`);
        }
      }
      holeElement.classList.toggle('theme-tour-hole-closed', !hole);
      card.dataset.mode = mode;

      place(card, hole, mode);
    };

    // a new step: its element into view first (smoothly unless motion is reduced)
    const first = findTargets(targets, together, rootRef.current)[0];
    if (first) {
      const rect = first.getBoundingClientRect();
      if (rect.top < 0 || rect.bottom > window.innerHeight) {
        first.scrollIntoView({
          block: rect.height > window.innerHeight * 0.6 ? 'start' : 'center',
          behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth',
        });
      }
    }
    measure();
    cardRef.current?.querySelector<HTMLElement>('[data-autofocus]')?.focus({ preventScroll: true });

    let frame = 0;
    const queue = () => {
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        measure();
      });
    };

    window.addEventListener('resize', queue);
    window.addEventListener('scroll', queue, true);
    // late lists, the balance poll and fonts all move things after the step showed
    const interval = setInterval(measure, 500);

    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener('resize', queue);
      window.removeEventListener('scroll', queue, true);
      clearInterval(interval);
    };
  }, [targetsKey, together, index]);

  // Escape is the way out, except on the acknowledgement
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !acknowledging) onSkip();
    };
    window.addEventListener('keydown', onKey);

    return () => window.removeEventListener('keydown', onKey);
  }, [acknowledging, onSkip]);

  if (!current) return null;

  const primaryLabel = last ? finishLabel : acknowledging ? acknowledgeLabel : t('elements.welcomeTour.next', {});

  return createPortal(
    <div ref={rootRef} className='theme-tour'>
      <div className='theme-tour-blocker' aria-hidden />
      <div ref={holeRef} className='theme-tour-hole theme-tour-hole-closed' aria-hidden />

      <section
        ref={cardRefs}
        className='theme-tour-card'
        data-mode='center'
        role='dialog'
        aria-modal='true'
        aria-labelledby='theme-tour-heading'
        aria-describedby='theme-tour-body'
      >
        {/* a dot per step, the current one widened into a bar */}
        <div className='theme-tour-progress'>
          <div className='theme-tour-dots' aria-hidden>
            {steps.map((entry, position) => (
              <span
                key={entry.key}
                className={classNames(
                  'theme-tour-dot',
                  position < index && 'theme-tour-dot-done',
                  position === index && 'theme-tour-dot-current',
                )}
              />
            ))}
          </div>
          <span>{t('elements.welcomeTour.progress', { step: index + 1, total: steps.length })}</span>
        </div>

        <div key={current.key} className='theme-tour-step'>
          <h2 id='theme-tour-heading' className='theme-tour-heading'>
            {current.heading}
          </h2>
          <p id='theme-tour-body' className='theme-tour-body'>
            {current.body}
          </p>
          {current.bullets && current.bullets.length > 0 && (
            <ul className='theme-tour-bullets'>
              {current.bullets.map((bullet) => (
                <li key={bullet}>{bullet}</li>
              ))}
            </ul>
          )}
        </div>

        {/* wizard order: Back and the way out at the start, the one primary action at the end */}
        <div className='theme-tour-actions'>
          <div className='theme-tour-actions-start'>
            {index > 0 && (
              <Button variant='default' onClick={() => onStep(index)}>
                {t('elements.welcomeTour.back', {})}
              </Button>
            )}
            {last
              ? laterLabel &&
                onLater && (
                  <button type='button' className='theme-link-action' onClick={onLater}>
                    {laterLabel}
                  </button>
                )
              : !acknowledging && (
                  <button type='button' className='theme-link-action' onClick={onSkip}>
                    {skipLabel}
                  </button>
                )}
          </div>
          <div className='theme-tour-actions-end'>
            <Button data-autofocus onClick={() => (last ? onFinish() : onStep(index + 2))}>
              {primaryLabel}
            </Button>
          </div>
        </div>
      </section>
    </div>,
    document.body,
  );
}
