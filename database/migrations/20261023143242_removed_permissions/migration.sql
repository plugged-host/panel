-- plugged.host: drops the permissions of the features this panel does not offer (devices, mounts,
-- managed database instances, backup groups, the private network, the per-server firewall and the
-- Extensions page; shared/src/permissions.rs REMOVED_*_PERMISSIONS), so the roles, API keys and
-- subusers that held one stay valid when they are edited.
UPDATE "roles" SET "admin_permissions" = ARRAY(
	SELECT p FROM unnest("admin_permissions") WITH ORDINALITY AS t(p, n)
	WHERE split_part(p, '.', 1) <> ALL (ARRAY['extensions', 'database-agent-hosts', 'database-agent-templates', 'mounts', 'devices'])
		AND p <> ALL (ARRAY['nodes.mounts', 'nodes.devices', 'nodes.database-agent-hosts', 'locations.database-agent-hosts', 'nodes.tunnel', 'servers.mounts', 'servers.devices', 'eggs.mounts', 'eggs.devices'])
	ORDER BY n
);
UPDATE "user_api_keys" SET "admin_permissions" = ARRAY(
	SELECT p FROM unnest("admin_permissions") WITH ORDINALITY AS t(p, n)
	WHERE split_part(p, '.', 1) <> ALL (ARRAY['extensions', 'database-agent-hosts', 'database-agent-templates', 'mounts', 'devices'])
		AND p <> ALL (ARRAY['nodes.mounts', 'nodes.devices', 'nodes.database-agent-hosts', 'locations.database-agent-hosts', 'nodes.tunnel', 'servers.mounts', 'servers.devices', 'eggs.mounts', 'eggs.devices'])
	ORDER BY n
);
UPDATE "roles" SET "server_permissions" = ARRAY(
	SELECT p FROM unnest("server_permissions") WITH ORDINALITY AS t(p, n)
	WHERE split_part(p, '.', 1) <> ALL (ARRAY['backup-groups', 'database-instances', 'firewall', 'connections', 'mounts', 'devices'])
	ORDER BY n
);
UPDATE "user_api_keys" SET "server_permissions" = ARRAY(
	SELECT p FROM unnest("server_permissions") WITH ORDINALITY AS t(p, n)
	WHERE split_part(p, '.', 1) <> ALL (ARRAY['backup-groups', 'database-instances', 'firewall', 'connections', 'mounts', 'devices'])
	ORDER BY n
);
UPDATE "server_subusers" SET "permissions" = ARRAY(
	SELECT p FROM unnest("permissions") WITH ORDINALITY AS t(p, n)
	WHERE split_part(p, '.', 1) <> ALL (ARRAY['backup-groups', 'database-instances', 'firewall', 'connections', 'mounts', 'devices'])
	ORDER BY n
);
