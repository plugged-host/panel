import type { BrowserHistory, Update } from 'history';
import { ReactNode, useLayoutEffect, useOptimistic, useState, useTransition } from 'react';
import { Router } from 'react-router';
import NavigationProgress from '@/elements/feedback/NavigationProgress.tsx';
import { PendingNavigationContext } from '@/providers/contexts/pendingNavigationContext.ts';

export default function TransitionHistoryRouter({
  history,
  children,
}: {
  history: BrowserHistory;
  children: ReactNode;
}) {
  const [state, setState] = useState<Update>({ action: history.action, location: history.location });
  const [isPending, startTransition] = useTransition();
  const [pendingPathname, setPendingPathname] = useOptimistic(state.location.pathname);

  useLayoutEffect(
    () =>
      history.listen((update) =>
        startTransition(() => {
          setPendingPathname(update.location.pathname);
          setState(update);
        }),
      ),
    [history],
  );

  return (
    <PendingNavigationContext.Provider value={isPending ? pendingPathname : null}>
      <Router location={state.location} navigationType={state.action} navigator={history as never}>
        {children}
      </Router>

      <NavigationProgress pending={isPending} />
    </PendingNavigationContext.Provider>
  );
}
