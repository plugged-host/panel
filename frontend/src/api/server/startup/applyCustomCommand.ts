// plugged.host: a startup command of the user's own, applied once the panel's checks let it through
// (docs/features/startup-commands.md)
import { axiosInstance } from '@/api/axios.ts';
import { transformKeysToCamelCase } from '@/lib/serialization/transformers.ts';

export interface CustomCommandRefusal {
  code: string;
  detail: string | null;
}

export interface CustomCommandVerdict {
  safe: boolean;
  startsTheGame: boolean;
  problems: string[];
  summary: string;
}

export interface CustomCommandResult {
  applied: boolean;
  refusal: CustomCommandRefusal | null;
  verdict: CustomCommandVerdict | null;
}

export default async (uuid: string, command: string): Promise<CustomCommandResult> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/startup/custom-command`, { command });
  return transformKeysToCamelCase(data) as CustomCommandResult;
};
