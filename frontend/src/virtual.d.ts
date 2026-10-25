declare module 'virtual:country-flags' {
  export const countryFlagCodes: string[];
}

declare module 'virtual:icon-manifest' {
  export const iconManifestUrl: string;
}

declare module 'cronstrue/locales/*';

// plugged.host: the offered languages that were built, with their locale loaders (vite-plugins/translations.ts)
declare module 'virtual:languages' {
  /** Code -> its own name, BCP 47 locale and direction, in the pickers' order. */
  export const languages: Record<string, { name: string; locale: string; dir: 'ltr' | 'rtl' }>;
  export const zodLocales: Record<string, () => Promise<unknown>>;
  export const monacoLocales: Record<string, { locale: string; load: () => Promise<unknown> }>;
  export const cronstrueLocales: Record<string, { name: string; load: () => Promise<unknown> }>;
}
