import classNames from 'classnames';
import { CSSProperties, forwardRef, ReactNode, Suspense } from 'react';
import { makeComponentHookable } from 'shared';
import '@/elements/feedback/spinner.css';

function Spinner({ size }: { size?: number }) {
  return (
    <span
      className='spinner'
      style={size !== undefined ? ({ '--loader-size': `${size}px` } as CSSProperties) : undefined}
      aria-label='Loading Spinner'
      data-testid='loader'
    />
  );
}

export const SpinnerLoader = forwardRef<HTMLSpanElement, { className?: string; style?: CSSProperties }>(
  ({ className, ...rest }, ref) => <span ref={ref} className={classNames('spinner', className)} {...rest} />,
);

export default makeComponentHookable(Spinner, {
  Centered: makeComponentHookable(({ size, className }: { size?: number; className?: string }) => (
    <div className={classNames('flex items-center justify-center py-6', className)}>
      <Spinner size={size} />
    </div>
  )),
  Suspense: makeComponentHookable(({ children, className }: { children: ReactNode; className?: string }) => (
    <Suspense
      fallback={
        <div className={classNames('flex items-center justify-center', className)}>
          <Spinner />
        </div>
      }
    >
      {children}
    </Suspense>
  )),
});
