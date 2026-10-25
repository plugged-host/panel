// plugged.host: the requests the first paint needs, started together at module load instead of one
// after another. `main.tsx` awaits settings and languages; the account and icons continue in the
// background and are ready by the time the shell needs them.

import getIcons from '@/api/getIcons.ts';
import getLanguages from '@/api/getLanguages.ts';
import getSettings from '@/api/getSettings.ts';
import getMe from '@/api/me/getMe.ts';

export const settingsPromise = getSettings();
export const languagesPromise = getLanguages();
export const iconsPromise = getIcons().catch(() => null);
export const mePromise = getMe().catch(() => null);
