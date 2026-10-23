// plugged.host: Admin › Sponsored Banners, one banner's form and image
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { z } from 'zod';
import createAdBanner from '@/api/admin/ad-banners/createAdBanner.ts';
import deleteAdBanner from '@/api/admin/ad-banners/deleteAdBanner.ts';
import updateAdBanner from '@/api/admin/ad-banners/updateAdBanner.ts';
import updateAdBannerImage from '@/api/admin/ad-banners/updateAdBannerImage.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import { AdminCan } from '@/elements/Can.tsx';
import AdminContentContainer from '@/elements/containers/AdminContentContainer.tsx';
import { type FieldDef, FormEngine, useFormEngine } from '@/elements/form-engine/index.ts';
import FileInput from '@/elements/input/FileInput.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import Text from '@/elements/typography/Text.tsx';
import Title from '@/elements/typography/Title.tsx';
import { bannerSlotSize } from '@/lib/ads.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { adminAdBannerSchema, adminAdBannerUpdateSchema } from '@/lib/schemas/admin/adBanners.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useResourceForm } from '@/plugins/resource/useResourceForm.ts';
import { useAdminCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type AdBanner = z.infer<typeof adminAdBannerSchema>;
type AdBannerFormValues = z.infer<typeof adminAdBannerUpdateSchema>;

const page = 'pages.admin.adBanners';

const adBannerEmptyFormValues: AdBannerFormValues = {
  advertiser: '',
  link: '',
  placement: 'slot',
  starts: null,
  ends: null,
  active: true,
};

const adBannerToFormValues = (banner: AdBanner): Partial<AdBannerFormValues> => ({
  advertiser: banner.advertiser,
  link: banner.link,
  placement: banner.placement,
  starts: banner.starts,
  ends: banner.ends,
  active: banner.active,
});

/** The banner's image: what it is now, and a new one (uploaded as soon as it is chosen). */
function AdBannerImage({ banner }: { banner: AdBanner }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const canUpdate = useAdminCan('ad-banners.update');
  const [uploading, setUploading] = useState(false);

  const shownAt = banner.placement === 'slot' ? bannerSlotSize(banner.width, banner.height) : null;

  const upload = (file: File | null) => {
    if (!file) return;
    setUploading(true);

    updateAdBannerImage(banner.uuid, file)
      .then(() => {
        addToast(t(`${page}.image.toast.updated`, {}), 'success');
        queryClient.invalidateQueries({ queryKey: queryKeys.admin.adBanners.detail(banner.uuid) });
        queryClient.invalidateQueries({ queryKey: queryKeys.admin.adBanners.all() });
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setUploading(false));
  };

  return (
    <Stack gap='sm' mt='xl'>
      <Title order={3} fz='md' fw={600}>
        {t(`${page}.image.title`, {})}
      </Title>
      <Text size='sm' c='dimmed'>
        {banner.placement === 'slot' ? t(`${page}.image.slotSizes`, {}) : t(`${page}.image.topSizes`, {})}
      </Text>

      {banner.imageUrl ? (
        <Stack gap='xs'>
          <img
            className='theme-ad-preview'
            src={banner.imageUrl}
            alt={banner.advertiser}
            width={shownAt?.[0] ?? banner.width}
            height={shownAt?.[1] ?? banner.height}
          />
          <Text size='sm' c='dimmed'>
            {t(`${page}.image.size`, { width: banner.width, height: banner.height })}{' '}
            {banner.placement === 'slot' &&
              (shownAt
                ? t(`${page}.image.shownAt`, { width: shownAt[0], height: shownAt[1] })
                : t(`${page}.image.noSlot`, {}))}
          </Text>
        </Stack>
      ) : (
        <Text size='sm'>{t(`${page}.image.none`, {})}</Text>
      )}

      {canUpdate && (
        <FileInput
          className='max-w-md'
          label={t(`${page}.image.upload`, {})}
          accept='image/png,image/jpeg,image/webp,image/gif'
          value={null}
          onChange={upload}
          disabled={uploading}
        />
      )}
    </Stack>
  );
}

export default ({ contextBanner }: { contextBanner?: AdBanner }) => {
  const { t } = useTranslations();
  const [openModal, setOpenModal] = useState<'delete' | null>(null);

  const form = useFormEngine<AdBannerFormValues>('admin.adBanners.createOrUpdate', {
    schema: adminAdBannerUpdateSchema,
    initialValues: adBannerEmptyFormValues,
  });

  const { loading, doCreateOrUpdate, doDelete } = useResourceForm<AdBannerFormValues, AdBanner>({
    form,
    createFn: () => createAdBanner(form.getTransformedValues()),
    updateFn: contextBanner ? () => updateAdBanner(contextBanner.uuid, form.getTransformedValues()) : undefined,
    deleteFn: contextBanner ? () => deleteAdBanner(contextBanner.uuid) : undefined,
    doUpdate: !!contextBanner,
    basePath: '/admin/ad-banners',
    resourceName: t(`${page}.resourceName`, {}),
  });

  useHydrateForm(form, contextBanner, adBannerToFormValues);

  const fields: FieldDef<AdBannerFormValues>[] = [
    { type: 'text', name: 'advertiser', label: t(`${page}.form.advertiser`, {}), required: true },
    {
      type: 'text',
      name: 'link',
      label: t(`${page}.form.link`, {}),
      description: t(`${page}.form.linkDescription`, {}),
      required: true,
    },
    {
      type: 'select',
      name: 'placement',
      label: t(`${page}.form.placement`, {}),
      description: t(`${page}.form.placementDescription`, {}),
      required: true,
      options: [
        { value: 'slot', label: t(`${page}.placement.slot`, {}) },
        { value: 'top', label: t(`${page}.placement.top`, {}) },
      ],
    },
    {
      type: 'switch',
      name: 'active',
      label: t(`${page}.form.active`, {}),
      description: t(`${page}.form.activeDescription`, {}),
    },
    { type: 'date', name: 'starts', label: t(`${page}.form.starts`, {}), props: { clearable: true } },
    {
      type: 'date',
      name: 'ends',
      label: t(`${page}.form.ends`, {}),
      description: t(`${page}.form.endsDescription`, {}),
      props: { clearable: true },
    },
  ];

  return (
    <AdminContentContainer
      title={
        contextBanner ? t(`${page}.titleUpdate`, { name: contextBanner.advertiser }) : t(`${page}.titleCreate`, {})
      }
      titleOrder={2}
    >
      <ConfirmationModal
        opened={openModal === 'delete'}
        onClose={() => setOpenModal(null)}
        title={t(`${page}.modal.delete.title`, {})}
        confirm={t('common.button.delete', {})}
        onConfirmed={doDelete}
      >
        {t('common.modal.delete.content', { name: form.getValues().advertiser }).md()}
      </ConfirmationModal>

      <form onSubmit={form.onSubmit(() => doCreateOrUpdate(false, queryKeys.admin.adBanners.all()))}>
        <FormEngine form={form} fields={fields} />

        {!contextBanner && (
          <Text size='sm' c='dimmed' mt='md'>
            {t(`${page}.image.afterCreate`, {})}
          </Text>
        )}

        <Group mt='md'>
          <AdminCan action={contextBanner ? 'ad-banners.update' : 'ad-banners.create'} cantSave>
            <Button type='submit' disabled={!form.isValid()} loading={loading}>
              {t('common.button.save', {})}
            </Button>
          </AdminCan>
          {contextBanner && (
            <AdminCan action='ad-banners.delete' cantDelete>
              <Button variant='default' onClick={() => setOpenModal('delete')} loading={loading}>
                {t('common.button.delete', {})}
              </Button>
            </AdminCan>
          )}
        </Group>
      </form>

      {contextBanner && <AdBannerImage banner={contextBanner} />}
    </AdminContentContainer>
  );
};
