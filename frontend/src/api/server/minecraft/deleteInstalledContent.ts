// plugged.host: delete an installed plugin or mod (docs/features/minecraft.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string, file: string, withData: boolean): Promise<void> => {
  await axiosInstance.delete(`/api/client/servers/${uuid}/minecraft/content/installed`, {
    data: { file, with_data: withData },
  });
};
