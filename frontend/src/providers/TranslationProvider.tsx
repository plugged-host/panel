// plugged.host: the offered languages that were built and their locale loaders (vite-plugins/translations.ts)
import { languages as builtLanguages, cronstrueLocales, monacoLocales, zodLocales } from 'virtual:languages';
import { DirectionProvider, Table, Text, Title, TitleOrder } from '@mantine/core';
import { Fragment, ReactNode, startTransition, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import Markdown, { Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import {
  getTranslationMapping,
  globalTranslationHandle,
  setGlobalTranslationHandle,
  TranslationContext,
  TranslationItem,
  TranslationItemRecord,
} from 'shared';
import { z } from 'zod';
import { $ZodConfig } from 'zod/v4/core';
import { axiosInstance } from '@/api/axios.ts';
import Anchor from '@/elements/typography/Anchor.tsx';
import Code from '@/elements/typography/Code.tsx';
import { intlLocale } from '@/lib/languages.ts'; // plugged.host: Latin digits
import { getGlobalStore, useGlobalStore } from '@/stores/global.ts';
import baseTranslations from '@/translations.ts';

// plugged.host: the zod, Monaco and cronstrue locale loaders come from `virtual:languages`, for the offered
// languages only (cronstrue: ar, de, fr, id, pt_BR, ru, tr, uk, vi; none for uz, whose cron descriptions stay English)
const monacoNlsCache: Record<string, string[] | undefined> = {};

// plugged.host: a language off the list (an old choice, an account from before the list) opens in the default
const offeredLanguage = (language: string | null | undefined): string | null =>
  language && Object.hasOwn(builtLanguages, language) ? language : null;
const defaultLanguage = (): string => offeredLanguage(getGlobalStore().settings.app.language) ?? 'en';

// zod's own regex message prints the raw pattern, schemas with a known rule override this via ruleMessage
z.config({
  customError: (issue) =>
    issue.code === 'invalid_format' && issue.format === 'regex' && globalTranslationHandle
      ? globalTranslationHandle.t('common.form.rule.invalidCharacters', {})
      : undefined,
});

type LanguageData = {
  items: TranslationItemRecord;
  translations: Record<string, string>;
};

declare global {
  interface String {
    md(): ReactNode;
  }

  var _VSCODE_NLS_MESSAGES: string[] | undefined;
  var _VSCODE_NLS_LANGUAGE: string | undefined;
}

const SafeMarkdownLink = ({ href, children }: { href?: string; children?: ReactNode }) => {
  if (href && /^(javascript|data|vbscript):/i.test(href)) {
    return <span>{children}</span>;
  }
  return (
    <Anchor href={href} inherit>
      {children}
    </Anchor>
  );
};

const detectBrowserLanguage = (languages: string[]): string | null => {
  for (const tag of navigator.languages) {
    if (languages.includes(tag)) return tag;

    try {
      const base = new Intl.Locale(tag).language;
      if (languages.includes(base)) return base;
    } catch {
      // ignore
    }
  }

  return null;
};

const Header =
  ({ order }: { order: TitleOrder }) =>
  (props: React.ComponentProps<typeof Title>) => <Title order={order} {...props} />;

export const markdownComponents: Components = {
  a: SafeMarkdownLink,
  p: ({ children }) => (
    <Text component='span' inherit>
      {children}
    </Text>
  ),
  h1: Header({ order: 1 }),
  h2: Header({ order: 2 }),
  h3: Header({ order: 3 }),
  h4: Header({ order: 4 }),
  h5: Header({ order: 5 }),
  h6: Header({ order: 6 }),
  pre: ({ children }) => <Fragment>{children}</Fragment>,
  code: ({ className, children }) => (
    <Code block={/language-/.test(className ?? '') || String(children).includes('\n')}>{children}</Code>
  ),
  table: ({ children }) => (
    <Table withTableBorder withColumnBorders>
      {children}
    </Table>
  ),
  thead: ({ children }) => <Table.Thead>{children}</Table.Thead>,
  tbody: ({ children }) => <Table.Tbody>{children}</Table.Tbody>,
  tr: ({ children }) => <Table.Tr>{children}</Table.Tr>,
  th: ({ children }) => <Table.Th>{children}</Table.Th>,
  td: ({ children }) => <Table.Td>{children}</Table.Td>,
  strong: ({ children }) => (
    <Text component='span' fw={700} inherit>
      {children}
    </Text>
  ),
  em: ({ children }) => (
    <Text component='span' td='italic' inherit>
      {children}
    </Text>
  ),
  ins: ({ children }) => (
    <Text component='span' td='underline' inherit>
      {children}
    </Text>
  ),
  del: ({ children }) => (
    <Text component='span' td='line-through' inherit>
      {children}
    </Text>
  ),
};

String.prototype.md = function (): ReactNode {
  return (
    <Markdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
      {this.toString()}
    </Markdown>
  );
};

const TranslationProvider = ({ children }: { children: ReactNode }) => {
  const [language, setLanguageState] = useState(
    offeredLanguage(localStorage.getItem('last_language')) ?? defaultLanguage(), // plugged.host
  );
  const [languageData, setLanguageData] = useState<LanguageData | null>(null);
  const languages = useGlobalStore((state) => state.languages);
  const pendingDetection = useRef(
    !offeredLanguage(localStorage.getItem('last_language')) && getGlobalStore().settings.user.allowChangingLanguage, // plugged.host
  );

  const setLanguage = useCallback((value: string) => {
    pendingDetection.current = false;
    setLanguageState(offeredLanguage(value) ?? defaultLanguage()); // plugged.host
  }, []);

  useEffect(() => {
    if (!pendingDetection.current || languages.length === 0) return;
    pendingDetection.current = false;

    const detected = detectBrowserLanguage(languages);
    if (detected) {
      setLanguageState(detected);
    }
  }, [languages]);

  // plugged.host: the three loaders read `virtual:languages` instead of globbing every locale
  const loadZod = async (lang: string) => {
    if (!zodLocales[lang]) {
      return;
    }

    const { default: locale } = (await zodLocales[lang]()) as {
      default: () => $ZodConfig;
    };

    z.config(locale());
  };

  const loadMonaco = async (lang: string) => {
    const nls = monacoLocales[lang];

    if (lang === 'en' || !nls) {
      globalThis._VSCODE_NLS_MESSAGES = undefined;
      globalThis._VSCODE_NLS_LANGUAGE = undefined;
      return;
    }

    if (!monacoNlsCache[nls.locale]) {
      await nls.load();
      monacoNlsCache[nls.locale] = globalThis._VSCODE_NLS_MESSAGES;
    }

    globalThis._VSCODE_NLS_MESSAGES = monacoNlsCache[nls.locale];
    globalThis._VSCODE_NLS_LANGUAGE = nls.locale;
  };

  const loadCronstrue = async (lang: string) => {
    if (lang === 'en') return;

    const locale = cronstrueLocales[lang];
    await locale?.load();

    // plugged.host: the pages ask cronstrue for the panel's code, so it names the file's locale (pt -> pt_BR),
    // or English where cronstrue has none. cronstrue is CommonJS: under Node's interop (this package is
    // "type": "module") `default` is its exports object, whose own `default` holds the locales.
    const { default: cronstrue } = await import('cronstrue');
    const descriptor = (cronstrue as unknown as { default?: typeof cronstrue }).default ?? cronstrue;
    descriptor.locales[lang] ??= descriptor.locales[locale?.name ?? 'en'];
  };

  useEffect(() => {
    let cancelled = false;

    startTransition(() => {
      if (language === 'en') {
        document.documentElement.lang = 'en';
        document.documentElement.dir = 'ltr';

        setLanguageData(null);
      } else {
        Promise.all([axiosInstance.get(`/translations/${language}.json`), loadCronstrue(language).catch(console.error)])
          .then(([{ data }]) => {
            if (cancelled) return;
            const result: LanguageData = {
              items: data[''].items,
              translations: data[''].translations,
            };

            for (const key in data) {
              if (key === '') continue;

              for (const item in data[key].items) {
                result.items[`${key}.${item}`] = data[key].items[item];
              }
              for (const translation in data[key].translations) {
                result.translations[`${key}.${translation}`] = data[key].translations[translation];
              }
            }

            result.translations = getTranslationMapping(result.translations);

            if (import.meta.env.DEV) {
              console.debug('Loaded language data', language, result);
            }

            // plugged.host: from the allow-list, which knows the direction (Firefox has no Intl.Locale#getTextInfo);
            // Mantine's DirectionProvider below follows the attribute
            document.documentElement.lang = builtLanguages[language]?.locale ?? language;
            document.documentElement.dir = builtLanguages[language]?.dir ?? 'ltr';

            setLanguageData(result);
          })
          .catch((err) => {
            if (cancelled) return;
            setLanguageState('en');
            console.error(err);
          });
      }

      loadZod(language);
      loadMonaco(language);
    });

    localStorage.setItem('last_language', language);

    return () => {
      cancelled = true;
    };
  }, [language]);

  const contextValue = useMemo(() => {
    // plugged.host: Latin digits (intlLocale)
    const numberFormat = new Intl.NumberFormat(intlLocale(language), { maximumFractionDigits: 20 });

    const formatValue = (value: unknown): string => {
      if (typeof value !== 'number') {
        return String(value);
      }

      return numberFormat.format(value);
    };

    const t = (key: string, values: Record<string, string | number>): string => {
      if (!languageData?.translations[key] && !baseTranslations.mapping[key as never]) {
        throw new Error(`Language key ${key} not found.`);
      }

      let translation = languageData?.translations[key] || (baseTranslations.mapping[key as never] as string);

      if (values) {
        Object.keys(values).forEach((placeholder) => {
          translation = translation.replaceAll(`{${placeholder}}`, formatValue(values[placeholder]));
        });
      }

      return translation;
    };

    const tReact = (key: string, values: Record<string, ReactNode>): ReactNode => {
      if (!languageData?.translations[key] && !baseTranslations.mapping[key as never]) {
        throw new Error(`Language key ${key} not found.`);
      }

      let translation = languageData?.translations[key] || (baseTranslations.mapping[key as never] as string);

      if (values) {
        const reactNodeKeys: string[] = [];
        Object.keys(values).forEach((placeholder) => {
          const value = values[placeholder];
          if (typeof value === 'string' || typeof value === 'number') {
            translation = translation.replaceAll(`{${placeholder}}`, formatValue(value));
          } else {
            reactNodeKeys.push(placeholder);
            translation = translation.replaceAll(`{${placeholder}}`, `%%${placeholder}%%`);
          }
        });

        if (reactNodeKeys.length === 0) {
          return (
            <Markdown
              components={{
                p: ({ children }) => <>{children}</>,
                a: SafeMarkdownLink,
              }}
            >
              {translation}
            </Markdown>
          );
        }

        const parts = translation.split(/(%%\w+%%)/g);
        return (
          <span>
            {parts.map((part, index) => {
              const match = part.match(/%%(\w+)%%/);
              if (match) {
                const placeholder = match[1];
                return <Fragment key={index}>{values[placeholder]}</Fragment>;
              }

              const leadingSpace = part.startsWith(' ') ? ' ' : '';
              const trailingSpace = part.endsWith(' ') ? ' ' : '';
              const trimmed = part.trim();
              if (!trimmed) {
                return <Fragment key={index}>{part}</Fragment>;
              }

              const hasMarkdown = /[*_`~[!#]/.test(trimmed);
              if (!hasMarkdown) {
                return <Fragment key={index}>{part}</Fragment>;
              }

              return (
                <Fragment key={index}>
                  {leadingSpace}
                  <Markdown
                    components={{
                      p: ({ children }) => <>{children}</>,
                      a: SafeMarkdownLink,
                    }}
                  >
                    {trimmed}
                  </Markdown>
                  {trailingSpace}
                </Fragment>
              );
            })}
          </span>
        );
      }

      return (
        <Markdown
          components={{
            p: ({ children }) => <>{children}</>,
            a: SafeMarkdownLink,
          }}
        >
          {translation}
        </Markdown>
      );
    };

    const tItem = (key: string, count: number): string => {
      if (!languageData?.items[key] && !baseTranslations.items[key as never]) {
        throw new Error(`Language item key ${key} not found.`);
      }

      const translationItem = languageData?.items[key] || baseTranslations.items[key as never];
      const rules = new Intl.PluralRules(language);
      // plugged.host: a plural form the language's file lacks falls back to English, like a missing string
      const translation =
        translationItem[rules.select(count)] ||
        (baseTranslations.items[key as never] as Partial<TranslationItem> | undefined)?.[rules.select(count)] ||
        translationItem.other;

      return translation.replaceAll('{count}', formatValue(count));
    };

    return { language, setLanguage, t, tReact, tItem };
  }, [language, languageData, setLanguage]);

  setGlobalTranslationHandle(contextValue);

  // plugged.host: Mantine's direction follows <html dir> (set above per language)
  return (
    <DirectionProvider>
      <TranslationContext.Provider value={contextValue}>{children}</TranslationContext.Provider>
    </DirectionProvider>
  );
};

export default TranslationProvider;
export { getTranslations, useTranslations } from './contexts/translationContext.ts';
