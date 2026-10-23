import { createContext, useContext } from 'react';

export const PendingNavigationContext = createContext<string | null>(null);

export const usePendingPathname = () => useContext(PendingNavigationContext);
