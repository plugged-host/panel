// plugged.host: a node's newest telemetry sample and its history (shared/src/node_telemetry.rs).
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminNodeTelemetryHistorySchema } from '@/lib/schemas/admin/nodes.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (nodeUuid: string, hours: number): Promise<z.infer<typeof adminNodeTelemetryHistorySchema>> => {
  const { data } = await axiosInstance.get(`/api/admin/nodes/${nodeUuid}/telemetry`, { params: { hours } });
  return parseFromApi(adminNodeTelemetryHistorySchema, data);
};
