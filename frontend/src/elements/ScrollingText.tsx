import { useDirection } from '@mantine/core';
import classNames from 'classnames';
import { CSSProperties, ReactNode, useEffect, useRef, useState } from 'react';

const MIN_DURATION = 6;
const MAX_DURATION = 30;

const SPEED = 20;

export default function ScrollingText({ children, className }: { children: ReactNode; className?: string }) {
  const containerRef = useRef<HTMLSpanElement>(null);
  const contentRef = useRef<HTMLSpanElement>(null);
  const [distance, setDistance] = useState(0);
  const [reducedMotion, setReducedMotion] = useState(false);
  const { dir } = useDirection(); // plugged.host: right to left the overflow is on the left

  useEffect(() => {
    const mql = window.matchMedia('(prefers-reduced-motion: reduce)');
    setReducedMotion(mql.matches);

    const handler = () => setReducedMotion(mql.matches);
    mql.addEventListener('change', handler);
    return () => mql.removeEventListener('change', handler);
  }, []);

  useEffect(() => {
    const container = containerRef.current;
    const content = contentRef.current;
    if (!container || !content) return;

    const measure = () => {
      const overflow = content.scrollWidth - container.clientWidth;
      setDistance(overflow > 1 ? overflow : 0);
    };

    measure();

    const observer = new ResizeObserver(measure);
    observer.observe(container);
    observer.observe(content);

    return () => observer.disconnect();
  }, [children]);

  const animate = distance > 0 && !reducedMotion;
  const duration = Math.min(Math.max(distance / SPEED, MIN_DURATION), MAX_DURATION);

  return (
    <span ref={containerRef} className={classNames('block overflow-hidden whitespace-nowrap min-w-0', className)}>
      <span
        ref={contentRef}
        className={classNames('inline-block align-top', animate ? 'animate-scrolling-text' : 'truncate max-w-full')}
        style={
          animate
            ? ({
                animationDuration: `${duration}s`,
                '--scrolling-text-distance': `${dir === 'rtl' ? '' : '-'}${distance}px`, // plugged.host
              } as CSSProperties)
            : undefined
        }
      >
        {children}
      </span>
    </span>
  );
}
