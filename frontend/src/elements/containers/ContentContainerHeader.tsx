import { faSearch } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { TitleOrder } from '@mantine/core';
import { Dispatch, ReactNode, SetStateAction } from 'react';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import TextInput from '../input/TextInput.tsx';
import { useNavbarPageHeader } from './useNavbarPageHeader.ts';

export interface ContentContainerHeaderProps {
  title: string;
  subtitle?: string;
  hideTitleComponent?: boolean;
  titleOrder?: TitleOrder;
  search?: string;
  setSearch?: Dispatch<SetStateAction<string>>;
  contentRight?: ReactNode;
  hoistToNavbar?: boolean;
}

/*
 * plugged.host: the Pelican panel's page header (Filament's ".fi-header" under Theme v2): the
 * title "text-xl font-semibold" with its subheading in gray-400 underneath, and the page's
 * actions at the end of the same row, the search first, as the table toolbar has it. Below `sm`
 * the actions go under the title. The shapes are ".ph-header*" in app.css.
 */
export default function ContentContainerHeader({
  title,
  subtitle,
  hideTitleComponent,
  titleOrder,
  search,
  setSearch,
  contentRight,
  hoistToNavbar = false,
}: ContentContainerHeaderProps) {
  const { t } = useTranslations();
  const hoisted = useNavbarPageHeader({ title, subtitle }, hoistToNavbar && !hideTitleComponent);

  if (hideTitleComponent) return null;

  const Heading = `h${titleOrder ?? 1}` as const;
  const hasActions = !!setSearch || !!contentRight;

  const titleBlock = (
    <div className={hoisted ? 'hidden min-w-0 lg:block' : 'min-w-0'}>
      <Heading className='ph-header-heading'>{title}</Heading>
      {subtitle ? <p className='ph-header-subheading'>{subtitle}</p> : null}
    </div>
  );

  return (
    <div className={hoisted && !hasActions ? 'hidden lg:block' : undefined}>
      <div className='ph-header'>
        {titleBlock}
        {hasActions && (
          <div className='ph-header-actions'>
            {setSearch && (
              <TextInput
                placeholder={t('common.input.search', {})}
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                leftSection={<FontAwesomeIcon icon={faSearch} />}
                className='ph-header-search'
              />
            )}
            {contentRight}
          </div>
        )}
      </div>
    </div>
  );
}
