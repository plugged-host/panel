// plugged.host: the foot of neo's chat (the Pelican panel's ".neo-foot"): what went wrong, the model
// picker and the usage bars, the images waiting to be sent, and the message field with its send
// button, which turns into a stop button while neo works.
import {
  faArrowUp,
  faCheck,
  faChevronUp,
  faCircleExclamation,
  faImage,
  faStop,
  faXmark,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQueryClient } from '@tanstack/react-query';
import classNames from 'classnames';
import {
  ClipboardEvent,
  DragEvent,
  FormEvent,
  KeyboardEvent,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import { useNavigate } from 'react-router';
import { httpErrorToHuman } from '@/api/axios.ts';
import { NeoIconButton } from '@/elements/neo/NeoHead.tsx';
import Menu from '@/elements/overlays/Menu.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import { NEO_IMAGE_TYPES, shrinkNeoImage } from '@/lib/neo.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import type { NeoInfo, NeoUsage, NeoWindow } from '@/lib/schemas/user/neo.ts';
import { useNeoText, useNeoUsage } from '@/plugins/useNeo.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { getNeoChat, NeoChat, NeoPlace, sendNeo, stopNeo, updateNeo, useNeoStore } from '@/stores/neo.ts';

// the largest picked image the browser shrinks (a phone photo is a few MB)
const MAX_IMAGE_BYTES = 20 * 1024 * 1024;
// the field grows with its text up to this height, then scrolls
const MAX_INPUT_PX = 160;

type Info = Omit<NeoInfo, 'conversation'>;

/** A window's share used, as a whole percent from 0 to 100. */
export function percentOf(window: NeoWindow): number {
  return Math.max(0, Math.min(100, Math.round(window.percent)));
}

/** A bar turns yellow near the limit and red at it. */
export function barClass(percent: number): string {
  return percent >= 95 ? 'theme-neo-bar-full' : percent >= 80 ? 'theme-neo-bar-warn' : '';
}

/** The windows in a fixed order: the short one, the long one, then whatever else noprofit.space has. */
export function orderedWindows(limits: Record<string, NeoWindow>): [string, NeoWindow][] {
  const order = (kind: string) => (kind === 'short' ? 0 : kind === 'long' ? 1 : 2);

  return Object.entries(limits).sort(([a], [b]) => order(a) - order(b) || a.localeCompare(b));
}

/** The model the chat uses: the person's pick while they may use it, else the default. */
export function currentModel(usage: NeoUsage, picked: string | null): string | null {
  return usage.models.some((model) => model.allowed && model.key === picked) ? picked : usage.defaultModel;
}

function ModelPicker({ usage, info, disabled }: { usage: NeoUsage; info: Info; disabled: boolean }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const text = useNeoText();
  const membershipEnabled = useGlobalStore((state) => state.settings.economy.membershipEnabled);
  const setOpen = useNeoStore((state) => state.setOpen);
  const [saving, setSaving] = useState(false);

  const current = currentModel(usage, info.preferences.model);
  const shown = usage.models.find((model) => model.key === current);

  const choose = (key: string) => {
    if (key === current) return;

    setSaving(true);
    updateNeo({ model: key })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setSaving(false));
  };

  return (
    <Menu position='top-start' width={272} withinPortal>
      <Menu.Target>
        <button type='button' className='theme-neo-model-btn' disabled={disabled || saving}>
          <span>{shown?.label ?? t('elements.neo.button.model', {})}</span>
          <FontAwesomeIcon icon={faChevronUp} />
        </button>
      </Menu.Target>
      <Menu.Dropdown aria-label={t('elements.neo.button.model', {})}>
        {usage.models.map((model) => {
          const locked = !model.allowed;
          const hint = [
            model.description,
            usage.mode === 'all' && model.multiplier > 1
              ? t('elements.neo.models.multiplier', { multiplier: model.multiplier })
              : null,
            usage.mode === 'models' && model.limited ? t('elements.neo.models.ownLimit', {}) : null,
            text.plan(model.access),
          ]
            .filter(Boolean)
            .join(' · ');

          return (
            <Menu.Item
              key={model.key}
              className={locked ? 'theme-neo-model-locked' : undefined}
              disabled={locked && !membershipEnabled}
              rightSection={model.key === current ? <FontAwesomeIcon icon={faCheck} className='theme-neo-ok' /> : null}
              onClick={() => {
                if (!locked) {
                  choose(model.key);
                  return;
                }

                // a model the membership does not open leads to where it is sold
                setOpen(false);
                navigate('/account/membership');
              }}
            >
              <div className='theme-neo-model-name'>{model.label}</div>
              {hint && <div className='theme-neo-model-hint'>{hint}</div>}
            </Menu.Item>
          );
        })}
      </Menu.Dropdown>
    </Menu>
  );
}

function Meter({ usage, info }: { usage: NeoUsage; info: Info }) {
  const { t } = useTranslations();
  const text = useNeoText();

  let windows: [string, NeoWindow][] = [];
  if (usage.mode === 'models') {
    // limits by model: only the picked model's own window, when it has one
    const model = usage.models.find((entry) => entry.key === currentModel(usage, info.preferences.model));
    if (model && !model.limited) {
      return <div className='theme-neo-meter theme-neo-meter-note'>{t('elements.neo.meter.noLimit', {})}</div>;
    }
    if (model?.window) windows = [['model', model.window]];
  } else if (usage.limits) {
    windows = orderedWindows(usage.limits);
  }

  if (windows.length === 0) return null;

  return (
    <div className='theme-neo-meter'>
      {windows.map(([kind, window]) => {
        const percent = percentOf(window);
        const label = text.windowLabel(window, true);

        return (
          <Tooltip
            key={kind}
            label={t('elements.neo.meter.title', {
              window: text.windowLabel(window),
              percent,
              time: text.resets(window),
            })}
            innerClassName='theme-neo-meter-item'
          >
            <span>{label}</span>
            <div className={classNames('theme-neo-bar', barClass(percent))}>
              <i style={{ width: `${percent}%` }} />
            </div>
            <span>{percent}%</span>
          </Tooltip>
        );
      })}
    </div>
  );
}

export default function NeoComposer({ place, info, chat }: { place: NeoPlace; info: Info; chat: NeoChat }) {
  const { t } = useTranslations();
  const text = useNeoText();
  const setDraft = useNeoStore((state) => state.setDraft);
  const setImages = useNeoStore((state) => state.setImages);
  const setFlash = useNeoStore((state) => state.setFlash);
  const queryClient = useQueryClient();
  const usage = useNeoUsage(true);
  const [preparing, setPreparing] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const wasWorking = useRef(chat.working);

  const key = place.key;

  // a finished turn used some of the limits: the bars catch up
  useEffect(() => {
    if (wasWorking.current && !chat.working) {
      void queryClient.invalidateQueries({ queryKey: queryKeys.user.neo.usage() });
    }
    wasWorking.current = chat.working;
  }, [chat.working, queryClient]);

  // the field follows its text, also when the text changes from outside (sent, given back)
  useLayoutEffect(() => {
    const input = inputRef.current;
    if (!input) return;

    input.style.height = 'auto';
    input.style.height = `${Math.min(input.scrollHeight, MAX_INPUT_PX)}px`;
  }, [chat.draft]);

  const say = (message: string) => setFlash(key, { code: null, params: null, message });

  const attach = async (files: FileList | File[] | null | undefined) => {
    const picked = Array.from(files ?? []).filter((file) => file.type.startsWith('image/'));
    if (picked.length === 0 || chat.working) return;

    const usable = picked.filter((file) => NEO_IMAGE_TYPES.includes(file.type) && file.size <= MAX_IMAGE_BYTES);
    if (usable.length < picked.length) {
      say(t('elements.neo.composer.imageType', {}));
    }

    const room = info.maxImages - getNeoChat(key).images.length;
    if (usable.length > room) {
      say(t('elements.neo.composer.imageLimit', { count: info.maxImages }));
    }
    if (room <= 0 || usable.length === 0) return;

    setPreparing(true);
    try {
      const shrunk = await Promise.all(usable.slice(0, room).map(shrinkNeoImage));
      setImages(key, [...getNeoChat(key).images, ...shrunk].slice(0, info.maxImages));
    } catch {
      say(t('elements.neo.composer.imageUnreadable', { name: text.name }));
    } finally {
      setPreparing(false);
    }
  };

  const submit = (event?: FormEvent) => {
    event?.preventDefault();
    if (chat.working || preparing) return;
    if (!chat.draft.trim() && chat.images.length === 0) return;

    void sendNeo(place, { text: chat.draft, images: chat.images }, true);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    // Enter sends on a keyboard; on a touch screen it is a new line and the button sends
    if (event.key !== 'Enter' || event.shiftKey || event.nativeEvent.isComposing) return;
    if (window.matchMedia('(pointer: coarse)').matches) return;

    event.preventDefault();
    submit();
  };

  const onPaste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    const files = Array.from(event.clipboardData.files);
    if (!files.some((file) => file.type.startsWith('image/'))) return;

    event.preventDefault();
    void attach(files);
  };

  const onDrop = (event: DragEvent<HTMLFormElement>) => {
    event.preventDefault();
    void attach(event.dataTransfer.files);
  };

  const flash = chat.flash ? (chat.flash.message ?? text.notice(chat.flash.code, chat.flash.params)) : null;
  const showTools = usage.data && (usage.data.models.length > 1 || usage.data.limits || usage.data.mode === 'models');

  return (
    <div className='theme-neo-foot'>
      {flash && (
        <div className='theme-neo-flash' role='alert'>
          <FontAwesomeIcon icon={faCircleExclamation} />
          <span>{flash}</span>
        </div>
      )}

      {usage.data && showTools && (
        <div className='theme-neo-tools'>
          {usage.data.models.length > 1 && <ModelPicker usage={usage.data} info={info} disabled={chat.working} />}
          <Meter usage={usage.data} info={info} />
        </div>
      )}

      {chat.images.length > 0 && (
        <div className='theme-neo-attachments'>
          {chat.images.map((src, index) => (
            <div key={src} className='theme-neo-attachment'>
              <img src={src} alt={t('elements.neo.message.image', {})} />
              <Tooltip label={t('elements.neo.button.removeImage', {})}>
                <button
                  type='button'
                  className='theme-neo-attachment-remove'
                  aria-label={t('elements.neo.button.removeImage', {})}
                  disabled={chat.working}
                  onClick={() =>
                    setImages(
                      key,
                      chat.images.filter((_, other) => other !== index),
                    )
                  }
                >
                  <FontAwesomeIcon icon={faXmark} />
                </button>
              </Tooltip>
            </div>
          ))}
        </div>
      )}

      <form
        className='theme-neo-compose'
        onSubmit={submit}
        onDragOver={(event) => event.preventDefault()}
        onDrop={onDrop}
      >
        <input
          ref={fileRef}
          type='file'
          accept={NEO_IMAGE_TYPES.join(',')}
          multiple
          hidden
          onChange={(event) => {
            void attach(event.currentTarget.files);
            event.currentTarget.value = '';
          }}
        />
        <NeoIconButton
          icon={faImage}
          label={t('elements.neo.button.attach', {})}
          onClick={() => fileRef.current?.click()}
          disabled={chat.working || preparing || chat.images.length >= info.maxImages}
        />
        <textarea
          ref={inputRef}
          className='theme-neo-input'
          rows={1}
          maxLength={info.maxMessageChars}
          value={chat.draft}
          placeholder={t('elements.neo.composer.placeholder', { name: text.name })}
          aria-label={t('elements.neo.composer.placeholder', { name: text.name })}
          onChange={(event) => setDraft(key, event.currentTarget.value)}
          onKeyDown={onKeyDown}
          onPaste={onPaste}
        />
        {chat.working ? (
          <Tooltip label={t('elements.neo.button.stop', {})}>
            <button
              type='button'
              className='theme-neo-send theme-neo-send-stop'
              aria-label={t('elements.neo.button.stop', {})}
              disabled={chat.stopping}
              onClick={() => stopNeo(place)}
            >
              <FontAwesomeIcon icon={faStop} />
            </button>
          </Tooltip>
        ) : (
          <Tooltip label={t('elements.neo.button.send', {})}>
            <button
              type='submit'
              className='theme-neo-send'
              aria-label={t('elements.neo.button.send', {})}
              disabled={preparing || (!chat.draft.trim() && chat.images.length === 0)}
            >
              <FontAwesomeIcon icon={faArrowUp} />
            </button>
          </Tooltip>
        )}
      </form>
    </div>
  );
}
