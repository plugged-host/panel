// plugged.host: the sidebar's nav rows share one background that slides between them, exactly
// like Altare's topbar (components/Navbar.tsx): the pill follows the pointer from row to row,
// glides back to the active row when the pointer leaves, and only the row under the pill reads
// bright. The rows themselves are transparent, so the movement is the whole hover feedback.
import classNames from 'classnames';
import {
  MouseEvent as ReactMouseEvent,
  ReactNode,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import { useLocation } from 'react-router';

type Indicator = {
  left: number;
  top: number;
  width: number;
  height: number;
};

const ROW = '.sidebar-link';
const ACTIVE_ROW = '.sidebar-link.active';

export default function SidebarNav({ children }: { children: ReactNode }) {
  const { pathname } = useLocation();
  const containerRef = useRef<HTMLDivElement>(null);
  const hoveredRef = useRef<Element | null>(null);
  const [indicator, setIndicator] = useState<Indicator | null>(null);
  const [hovering, setHovering] = useState(false);

  const measure = useCallback((row: Element | null) => {
    const container = containerRef.current;
    if (!container || !row || !container.contains(row)) {
      setIndicator((current) => (current === null ? current : null));
      return;
    }

    const box = row.getBoundingClientRect();
    const parent = container.getBoundingClientRect();
    const next = {
      left: box.left - parent.left,
      top: box.top - parent.top,
      width: box.width,
      height: box.height,
    };

    setIndicator((current) =>
      current &&
      current.left === next.left &&
      current.top === next.top &&
      current.width === next.width &&
      current.height === next.height
        ? current
        : next,
    );
  }, []);

  const showActive = useCallback(() => {
    hoveredRef.current = null;
    setHovering(false);
    measure(containerRef.current?.querySelector(ACTIVE_ROW) ?? null);
  }, [measure]);

  useLayoutEffect(() => {
    showActive();
  }, [pathname, showActive]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return;
    }

    // A group opening or closing moves the rows after it, and the sidebar resizing moves all of
    // them; keep the pill on whichever row it is meant to be on.
    const observer = new ResizeObserver(() => measure(hoveredRef.current ?? container.querySelector(ACTIVE_ROW)));
    observer.observe(container);
    return () => observer.disconnect();
  }, [measure]);

  const onMouseOver = (event: ReactMouseEvent<HTMLDivElement>) => {
    const row = (event.target as Element).closest(ROW);
    if (!row || row === hoveredRef.current || !containerRef.current?.contains(row)) {
      return;
    }

    hoveredRef.current = row;
    setHovering(true);
    measure(row);
  };

  return (
    <div
      ref={containerRef}
      className={classNames('sidebar-nav', hovering && 'is-hovering')}
      onMouseOver={onMouseOver}
      onMouseLeave={showActive}
    >
      {indicator && (
        <span
          aria-hidden='true'
          className='sidebar-nav-indicator'
          style={{
            left: indicator.left,
            top: indicator.top,
            width: indicator.width,
            height: indicator.height,
          }}
        />
      )}
      {children}
    </div>
  );
}
