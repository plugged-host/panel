-- plugged.host: paid servers are called after the panel now ("<panel name> Paid") while the name
-- setting is empty, its new default (docs/features/dedicated.md). The old default, Dedicated,
-- goes so the new one shows.
DELETE FROM "settings" WHERE "key" = 'games::dedicated_name' AND "value" = 'Dedicated';
