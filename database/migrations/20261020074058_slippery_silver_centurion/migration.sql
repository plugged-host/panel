DROP INDEX "allocations_node_uuid_idx";
DROP INDEX "server_activities_server_uuid_idx";
CREATE INDEX "allocations_node_uuid_ip_inet_port_idx" ON "node_allocations" ("node_uuid","ip","port");
CREATE INDEX "server_activities_server_uuid_created_idx" ON "server_activities" ("server_uuid","created");
CREATE INDEX "server_backups_node_uuid_created_idx" ON "server_backups" ("node_uuid","created") WHERE "deleted" IS NULL;
CREATE INDEX "servers_created_idx" ON "servers" ("created");
CREATE INDEX "users_created_idx" ON "users" ("created");