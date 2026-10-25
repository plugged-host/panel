// plugged.host: try a failed server of the node's failovers again
import { axiosInstance } from '@/api/axios.ts';

export default async (nodeUuid: string, serverFailoverUuid: string): Promise<void> => {
  await axiosInstance.post(`/api/admin/nodes/${nodeUuid}/failover/resume`, {
    server_failover_uuid: serverFailoverUuid,
  });
};
