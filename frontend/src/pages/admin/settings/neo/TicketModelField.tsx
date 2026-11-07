// plugged.host: the model slot neo uses in support tickets (shared/src/tickets.rs).
import { UseFormReturnType } from '@mantine/form';
import Select from '@/elements/input/Select.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import type { NeoValues } from './neoFormValues.ts';

const page = 'pages.admin.settings.tabs.neo.page';

export default function TicketModelField({ form }: { form: UseFormReturnType<NeoValues> }) {
  const { t } = useTranslations();
  const providers = useAdminStore((state) => state.neo.providers);
  const value = form.getValues().ticketModel;

  const data = [
    { value: '0', label: t(`${page}.form.ticketModelDefault`, {}) },
    ...providers
      .filter((provider) => provider.enabled)
      .sort((a, b) => a.slot - b.slot)
      .map((provider) => ({
        value: String(provider.slot),
        label: provider.label.trim()
          ? t(`${page}.form.ticketModelSlot`, { slot: provider.slot, label: provider.label.trim() })
          : t(`${page}.form.ticketModelSlotPlain`, { slot: provider.slot }),
      })),
  ];

  return (
    <Select
      label={t(`${page}.form.ticketModel`, {})}
      description={t(`${page}.form.ticketModelDescription`, {})}
      data={data}
      value={String(value)}
      onChange={(next) => next && form.setFieldValue('ticketModel', Number(next))}
      allowDeselect={false}
    />
  );
}
