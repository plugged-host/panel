// plugged.host: the console's search without upstream's header row (the Pelican console has none): the same field and arrows, opened by its shortcut or the "Search Console" quick action, at the terminal's top end
import { faArrowDown, faArrowUp } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { SearchAddon } from '@xterm/addon-search';
import { RefObject } from 'react';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Popover from '@/elements/overlays/Popover.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default function ConsoleSearch({
  opened,
  onChange,
  searchText,
  setSearchText,
  searchAddonRef,
}: {
  opened: boolean;
  onChange: (opened: boolean) => void;
  searchText: string;
  setSearchText: (value: string) => void;
  searchAddonRef: RefObject<SearchAddon | null>;
}) {
  const { t } = useTranslations();

  return (
    <Popover trapFocus opened={opened} onChange={onChange} position='bottom-end'>
      <Popover.Target>
        <span aria-hidden className='absolute top-1 end-3 size-0' />
      </Popover.Target>
      <Popover.Dropdown className='flex flex-row gap-2' p='xs'>
        <TextInput
          placeholder={t('common.input.search', {})}
          aria-label={t('pages.server.console.tooltip.search', {})}
          value={searchText}
          onChange={(e) => {
            setSearchText(e.currentTarget.value);
            searchAddonRef.current?.findNext(e.currentTarget.value, {
              incremental: true,
            });
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              if (e.shiftKey) {
                searchAddonRef.current?.findPrevious(searchText);
              } else {
                searchAddonRef.current?.findNext(searchText);
              }
            }
          }}
        />
        <ActionIcon
          size='input-sm'
          variant='subtle'
          color='gray'
          aria-label={t('pages.server.console.search.previous', {})}
          onClick={() => searchAddonRef.current?.findPrevious(searchText)}
        >
          <FontAwesomeIcon icon={faArrowUp} />
        </ActionIcon>
        <ActionIcon
          size='input-sm'
          variant='subtle'
          color='gray'
          aria-label={t('pages.server.console.search.next', {})}
          onClick={() => searchAddonRef.current?.findNext(searchText)}
        >
          <FontAwesomeIcon icon={faArrowDown} />
        </ActionIcon>
      </Popover.Dropdown>
    </Popover>
  );
}
