// plugged.host: Admin › Sponsored Banners, the banner's image (PNG, JPEG, WebP or GIF)
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string, image: File): Promise<string> => {
  const { data } = await axiosInstance.put(`/api/admin/ad-banners/${uuid}/image`, image, {
    headers: {
      'Content-Type': image.type || 'application/octet-stream',
    },
  });
  return data.image_url;
};
