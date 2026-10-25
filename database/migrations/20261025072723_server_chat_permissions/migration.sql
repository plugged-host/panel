-- plugged.host: the server chat (docs/features/server-chat.md). Every subuser who could see the
-- server keeps seeing it, and can now read and write the chat; owners have every permission
-- already.
UPDATE server_subusers
SET permissions = array_append(permissions, 'chat.read')
WHERE NOT ('chat.read' = ANY (permissions));

UPDATE server_subusers
SET permissions = array_append(permissions, 'chat.write')
WHERE NOT ('chat.write' = ANY (permissions));
