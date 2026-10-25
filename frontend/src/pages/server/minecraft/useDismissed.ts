// plugged.host: a callout the reader closed stays closed for the browser tab (docs/features/minecraft.md)
import { useState } from 'react';

function read(key: string) {
  try {
    return window.sessionStorage.getItem(key) !== null;
  } catch {
    return false;
  }
}

/** `key` names one occurrence (a server and a start time), so the next one shows again. */
export function useDismissed(key: string | null): [boolean, () => void] {
  const [dismissed, setDismissed] = useState<string[]>([]);

  const isDismissed = key !== null && (dismissed.includes(key) || read(key));
  const dismiss = () => {
    if (key === null) return;
    setDismissed((keys) => [...keys, key]);
    try {
      window.sessionStorage.setItem(key, '1');
    } catch {
      // private mode or blocked storage: closed until the page reloads
    }
  };

  return [isDismissed, dismiss];
}
