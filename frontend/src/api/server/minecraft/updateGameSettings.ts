// plugged.host: change values of one settings file (docs/features/minecraft.md)
import { axiosInstance } from '@/api/axios.ts';

export interface GameSettingsEdit {
  path: string[];
  value: boolean | number | string | null;
}

/** Returns how many values changed. */
export default async (uuid: string, file: string, edits: GameSettingsEdit[]): Promise<number> => {
  const { data } = await axiosInstance.put(`/api/client/servers/${uuid}/minecraft/settings`, { file, edits });
  return data.changed;
};
