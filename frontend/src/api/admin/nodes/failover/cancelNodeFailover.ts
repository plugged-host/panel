// plugged.host: stop a node's failover; servers already moved stay where they are
import { axiosInstance } from '@/api/axios.ts';

export default async (nodeUuid: string): Promise<void> => {
  await axiosInstance.post(`/api/admin/nodes/${nodeUuid}/failover/cancel`);
};
