import { RefObject, useCallback, useEffect, useRef, useState } from 'react';

interface FileTreeResizeHandleProps {
  gridRef: RefObject<HTMLDivElement | null>;
  treeRef: RefObject<HTMLDivElement | null>;
  label: string;
  onResize: (width: number | null) => void;
}

interface ResizeSession {
  grid: HTMLDivElement;
  startX: number;
  startWidth: number;
  minimumWidth: number;
  maximumWidth: number;
  nextWidth: number;
  frame: number | null;
}

const getWidthBounds = (grid: HTMLDivElement) => {
  const rem = Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
  const gap = Number.parseFloat(getComputedStyle(grid).columnGap) || 0;
  const minimumWidth = 14 * rem;

  return { minimumWidth, maximumWidth: Math.max(minimumWidth, grid.clientWidth - gap - 20 * rem) };
};

export default function FileTreeResizeHandle({ gridRef, treeRef, label, onResize }: FileTreeResizeHandleProps) {
  const resizeSession = useRef<ResizeSession | null>(null);
  const [resizing, setResizing] = useState(false);

  useEffect(() => {
    if (!resizing) return;

    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';

    return () => {
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
  }, [resizing]);

  const finishResize = useCallback(() => {
    const session = resizeSession.current;
    if (!session) return;

    if (session.frame !== null) cancelAnimationFrame(session.frame);
    session.grid.style.setProperty('--file-manager-tree-width', `${session.nextWidth}px`);
    session.grid.style.removeProperty('transition');
    onResize(Math.round(session.nextWidth));
    resizeSession.current = null;
    setResizing(false);
  }, [onResize]);

  useEffect(() => finishResize, [finishResize]);

  const resize = (event: React.PointerEvent) => {
    const session = resizeSession.current;
    if (!session) return;

    session.nextWidth = Math.min(
      session.maximumWidth,
      Math.max(session.minimumWidth, session.startWidth + event.clientX - session.startX),
    );
    if (session.frame !== null) return;

    session.frame = requestAnimationFrame(() => {
      session.frame = null;
      session.grid.style.setProperty('--file-manager-tree-width', `${session.nextWidth}px`);
    });
  };

  const startResize = (event: React.PointerEvent) => {
    const grid = gridRef.current;
    const tree = treeRef.current;
    if (!grid || !tree) return;

    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    const startWidth = tree.getBoundingClientRect().width;
    grid.style.transition = 'none';

    resizeSession.current = {
      grid,
      startX: event.clientX,
      startWidth,
      ...getWidthBounds(grid),
      nextWidth: startWidth,
      frame: null,
    };
    setResizing(true);
  };

  const resizeWithKeyboard = (event: React.KeyboardEvent) => {
    if (!['ArrowLeft', 'ArrowRight', 'Home'].includes(event.key)) return;

    event.preventDefault();
    if (event.key === 'Home') {
      onResize(null);
      return;
    }

    const grid = gridRef.current;
    const tree = treeRef.current;
    if (!grid || !tree) return;

    const { minimumWidth, maximumWidth } = getWidthBounds(grid);
    const step = grid.clientWidth * 0.05;
    const width = tree.getBoundingClientRect().width + (event.key === 'ArrowLeft' ? -step : step);
    onResize(Math.round(Math.min(maximumWidth, Math.max(minimumWidth, width))));
  };

  return (
    <div
      role='separator'
      tabIndex={0}
      aria-orientation='vertical'
      aria-label={label}
      data-file-manager-tree-resize-handle
      className='file-manager-editor-resize-handle absolute inset-y-0 -left-(--file-manager-workspace-gap) col-start-2 row-start-1 w-(--file-manager-workspace-gap) cursor-col-resize touch-none focus-visible:outline-none'
      onPointerDown={startResize}
      onPointerMove={resize}
      onPointerUp={finishResize}
      onPointerCancel={finishResize}
      onKeyDown={resizeWithKeyboard}
    />
  );
}
